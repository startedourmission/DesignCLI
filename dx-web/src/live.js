// 라이브 동기화 — 데몬(dx-daemon)이 유일한 순서 결정자.
//
// 양방향 모델: 로컬 편집을 즉시 적용하지 않는다. 모든 편집(웹·CLI)은 데몬에 POST되고,
// 데몬이 seq를 붙여 broadcast하면 그때 각 클라가 wasm editor에 적용한다. 자기 편집도
// broadcast를 거쳐 돌아오므로 echo 중복·순서꼬임이 없고, 모든 클라가 동일 seq 스트림을
// 소비해 상태가 결정적으로 일치한다(.dxpkg 스냅샷 + 동일 Action 재적용).

import { Editor } from "./wasm/pkg/dcli_wasm.js";

const b64ToBytes = (b64) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));

/**
 * 라이브 모드 연결.
 * @param {App} app  쓰기 funnel을 라이브로 전환할 App
 * @param {string} docId  데몬 문서 id (?doc=<id>)
 * @returns {Promise<LiveLink>}
 */
export async function connectLive(app, docId) {
  const link = new LiveLink(app, docId);
  await link.start();
  return link;
}

class LiveLink {
  constructor(app, docId) {
    this.app = app;
    this.docId = docId;
    this.lastSeq = 0;
    this.ws = null;
    this._resyncing = false;
    this._pendingOptimistic = [];
    this._inflight = new Set(); // 진행 중 apply Promise들 — flush()가 대기.
  }

  async start() {
    await this._loadSnapshot();
    this._openSocket();
    // App을 라이브 모드로 전환: 쓰기는 이 link를 통해 데몬으로.
    this.app.setLive(this);
  }

  // 현재 데몬 상태를 .dxpkg로 받아 editor를 교체하고 seq를 동기화.
  async _loadSnapshot() {
    let seq;
    let bytes;
    const bin = await fetch(`/doc/${this.docId}/snapshot.bin`);
    if (bin.ok) {
      seq = Number(bin.headers.get("x-dx-seq") || 0);
      bytes = new Uint8Array(await bin.arrayBuffer());
    } else {
      const r = await fetch(`/doc/${this.docId}/snapshot`);
      if (!r.ok) throw new Error(`snapshot 실패: ${r.status} ${await r.text()}`);
      const json = await r.json();
      seq = json.seq;
      bytes = b64ToBytes(json.dxpkg_base64);
    }
    const ed = Editor.from_dxpkg(bytes);
    this._pendingOptimistic = [];
    this.app.replaceEditor(ed); // 렌더러/패널까지 새 editor로 재배선
    this.lastSeq = seq;
  }

  _openSocket() {
    const proto = location.protocol === "https:" ? "wss" : "ws";
    const ws = new WebSocket(`${proto}://${location.host}/doc/${this.docId}/live`);
    this.ws = ws;
    ws.onmessage = (ev) => this._onMessage(ev);
    ws.onclose = () => this._scheduleReconnect();
    ws.onerror = () => ws.close();
  }

  _scheduleReconnect() {
    if (this._resyncing) return;
    // 끊기면 잠깐 뒤 재연결(재연결 시 snapshot부터 다시 받아 누락 메움).
    setTimeout(() => this._resync(), 800);
  }

  async _resync() {
    if (this._resyncing) return;
    this._resyncing = true;
    try {
      await this._loadSnapshot();
      this._openSocket();
    } catch (e) {
      console.warn("재동기 실패, 재시도 예약:", e);
      setTimeout(() => this._resync(), 1500);
    } finally {
      this._resyncing = false;
    }
  }

  _onMessage(ev) {
    let msg;
    try {
      msg = JSON.parse(ev.data);
    } catch {
      return;
    }
    switch (msg.type) {
      case "hello":
        // 데몬의 현재 seq가 내 snapshot seq보다 앞서면 그 사이 편집을 놓친 것 → 재동기.
        if (msg.seq !== this.lastSeq) this._resync();
        break;
      case "ops":
      case "undo":
      case "redo":
        this._applyRemote(msg);
        break;
      case "lagged":
        // 데몬 버퍼 초과로 밀림 → snapshot 재동기.
        this._resync();
        break;
    }
  }

  // 데몬이 정한 순서대로 로컬 editor에 적용. seq 연속성이 깨지면 재동기.
  _applyRemote(msg) {
    if (msg.seq !== this.lastSeq + 1) {
      this._resync();
      return;
    }
    const ed = this.app.editor;
    let optimisticEcho = false;
    try {
      if (msg.type === "ops") {
        const encoded = JSON.stringify(msg.actions);
        if (this._pendingOptimistic[0] === encoded) {
          this._pendingOptimistic.shift();
          optimisticEcho = true;
        } else if (this._pendingOptimistic.length) {
          this._resync();
          return;
        } else {
          ed.apply_actions(encoded);
        }
      } else if (msg.type === "undo") {
        this._pendingOptimistic = [];
        ed.undo();
      } else if (msg.type === "redo") {
        this._pendingOptimistic = [];
        ed.redo();
      }
    } catch (e) {
      console.error("원격 적용 실패:", e);
      this._resync();
      return;
    }
    this.lastSeq = msg.seq;
    if (optimisticEcho) this.app.afterOptimisticAck();
    else this.app.afterRemoteApply();
  }

  // ---- App이 호출하는 송신 funnel (로컬 적용 안 함, 데몬에만 보냄) ----
  async sendApply(actions, encoded = JSON.stringify(actions)) {
    this._pendingOptimistic.push(encoded);
    // 진행 중 apply 추적 — 네비게이션 전 flush()가 이들을 기다려 마지막 편집 유실 방지.
    const inflight = (async () => {
      const r = await fetch(`/doc/${this.docId}/apply`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: encoded,
      });
      if (!r.ok) {
        this._pendingOptimistic = this._pendingOptimistic.filter((v) => v !== encoded);
        console.error("apply 전송 실패:", r.status, await r.text());
        this._resync();
        return;
      }
      // 엔진이 거부한 편집(예: PNG 디코드 실패)을 조용히 삼키지 않는다.
      const j = await r.json().catch(() => null);
      if (j?.result && j.result.ok === false) {
        this._pendingOptimistic = this._pendingOptimistic.filter((v) => v !== encoded);
        const msg = j.result.issues?.[0]?.message || "적용 실패";
        console.error("[live] 편집 거부:", j.result.issues);
        alert(`편집 실패: ${msg}`);
        this._resync();
      }
    })();
    this._inflight.add(inflight);
    inflight.finally(() => this._inflight.delete(inflight));
    return inflight;
  }

  /** 진행 중인 모든 apply가 데몬에 도달할 때까지 대기 — 프로젝트 전환/네비게이션 전 호출.
   *  데몬은 apply가 도달하면 in-memory로 보존하고 1.5s 디바운스로 디스크 저장하므로,
   *  flush 후 네비게이션하면 in-flight POST 취소로 인한 마지막 편집 유실이 없다. */
  async flush() {
    await Promise.allSettled([...this._inflight]);
  }
  async sendUndo() {
    await fetch(`/doc/${this.docId}/undo`, { method: "POST" });
  }
  async sendRedo() {
    await fetch(`/doc/${this.docId}/redo`, { method: "POST" });
  }
}
