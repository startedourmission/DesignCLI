// AppController — Editor(wasm) 핸들이 유일 진실원. 모든 쓰기는 여기를 통과한다.
// 패널은 editor.layers() 읽기 전용, 쓰기는 apply()로만(이중 상태관리 금지).

import * as B from "./bridge.js";

export class Renderer {
  constructor(editor, canvas) {
    this.editor = editor;
    this._dirty = false;
    this._raf = 0;
    this.canvas = canvas; // setter가 ctx까지 함께 잡는다.
  }
  // 캔버스 교체 시 ctx도 반드시 같이 갱신(dx-canvas가 실제 캔버스를 주입할 때).
  set canvas(c) {
    this._canvas = c;
    this.ctx = c.getContext("2d", { alpha: true });
    this.ctx.imageSmoothingEnabled = false;
  }
  get canvas() {
    return this._canvas;
  }
  resize() {
    this.canvas.width = this.editor.width();
    this.canvas.height = this.editor.height();
    this.markDirty();
  }
  markDirty() {
    this._dirty = true;
    if (!this._raf) this._raf = requestAnimationFrame(() => this._frame());
  }
  _frame() {
    this._raf = 0;
    if (!this._dirty) return;
    this._dirty = false;
    // 매 프레임 새 복사본을 받는다(메모리 detach 무관 — wasm이 소유 복사본 반환).
    const buf = this.editor.composite_rgba();
    const img = new ImageData(buf, this.editor.width(), this.editor.height());
    this.ctx.putImageData(img, 0, 0);
  }
}

export class App extends EventTarget {
  constructor(editor, renderer) {
    super();
    this.editor = editor;
    this.renderer = renderer;
    this.live = null; // 라이브 모드면 LiveLink. 설정 시 쓰기가 데몬을 경유.
    this.selectedId = null; // 선택툴이 고른 레이어 node id(null=선택 없음).
  }

  /** 레이어 선택(선택툴). null이면 해제. changed로 캔버스·패널 동기. */
  select(id) {
    this.selectedId = id;
    this._notify();
  }

  /** 선택된 레이어 객체(top-to-bottom 목록에서 조회). 없으면 null. */
  getSelected() {
    if (this.selectedId == null) return null;
    return this.layers().find((l) => l.id === this.selectedId) || null;
  }

  /** 캔버스 좌표 hit-test → 최상위 레이어 id(없으면 null). */
  hitTest(x, y) {
    const id = this.editor.hit_test(Math.floor(x), Math.floor(y));
    return id < 0 ? null : id;
  }

  /** 레이어 불투명 바운드 [x,y,w,h](offset 반영) 또는 null. */
  layerBounds(id) {
    return JSON.parse(this.editor.layer_bounds(id));
  }

  /** 레이어 복제(Cmd+D). 데몬/로컬 공통 — DuplicateLayer Action. */
  duplicate(id) {
    if (id == null) return;
    this.apply([B.duplicateLayer(id)]);
  }

  /** 선택 레이어 offset 상대이동(화살표 nudge). */
  nudge(id, dx, dy) {
    const l = this.layers().find((v) => v.id === id);
    if (!l) return;
    const [ox, oy] = l.offset ?? [0, 0];
    this.apply([B.setOffset(id, [ox + dx, oy + dy])]);
  }

  /** 레이어의 표시 트랜스폼(엔진과 동일 수학: 표면 중심 기준 scale→rotate→offset). */
  xformOf(l) {
    const W = this.editor.width(), H = this.editor.height();
    const c = { x: W / 2, y: H / 2 };
    const rad = ((l.rotation ?? 0) * Math.PI) / 180;
    const cos = Math.cos(rad), sin = Math.sin(rad);
    const [sx, sy] = l.scale ?? [1, 1];
    const [ox, oy] = l.offset ?? [0, 0];
    return {
      cos, sin, sx, sy, ox, oy, c,
      /** src 좌표 → 캔버스 좌표 */
      fwd(px, py) {
        const vx = (px - c.x) * sx, vy = (py - c.y) * sy;
        return { x: cos * vx - sin * vy + c.x + ox, y: sin * vx + cos * vy + c.y + oy };
      },
      /** 캔버스 좌표 → src 좌표 */
      inv(qx, qy) {
        const ax = qx - ox - c.x, ay = qy - oy - c.y;
        const rx = cos * ax + sin * ay, ry = -sin * ax + cos * ay;
        return { x: rx / sx + c.x, y: ry / sy + c.y };
      },
      /** 트랜스폼 중심(회전·스케일 고정점)의 캔버스 좌표 */
      center() { return { x: c.x + ox, y: c.y + oy }; },
    };
  }

  /** 레이어의 변환 후 AABB(캔버스 좌표) {x,y,w,h} 또는 null. 정렬·표시용. */
  displayAABB(l) {
    const b = this.layerBounds(l.id);
    if (!b) return null;
    const t = this.xformOf(l);
    const pts = [
      t.fwd(b[0], b[1]), t.fwd(b[0] + b[2], b[1]),
      t.fwd(b[0], b[1] + b[3]), t.fwd(b[0] + b[2], b[1] + b[3]),
    ];
    const xs = pts.map((p) => p.x), ys = pts.map((p) => p.y);
    const x = Math.min(...xs), y = Math.min(...ys);
    return { x, y, w: Math.max(...xs) - x, h: Math.max(...ys) - y };
  }

  /** 선택 레이어를 문서 기준 정렬: left|center-h|right|top|center-v|bottom */
  align(id, mode) {
    const l = this.layers().find((v) => v.id === id);
    if (!l) return;
    const box = this.displayAABB(l);
    if (!box) return;
    const W = this.editor.width(), H = this.editor.height();
    const [ox, oy] = l.offset ?? [0, 0];
    let dx = 0, dy = 0;
    if (mode === "left") dx = -box.x;
    else if (mode === "center-h") dx = (W - box.w) / 2 - box.x;
    else if (mode === "right") dx = W - box.w - box.x;
    else if (mode === "top") dy = -box.y;
    else if (mode === "center-v") dy = (H - box.h) / 2 - box.y;
    else if (mode === "bottom") dy = H - box.h - box.y;
    this.apply([B.setOffset(id, [Math.round(ox + dx), Math.round(oy + dy)])]);
  }

  /** bottom-to-top 순서(엔진 order). moveLayer의 to는 이 인덱스 기준. */
  orderBottomToTop() {
    const j = JSON.parse(this.editor.layers());
    return j.layers.map((l) => l.id); // dto layer_list_json = bottom-to-top
  }

  /** 레이어를 한 칸 위(앞)로 — z-order 상승. 맨 위면 무시. */
  raise(id) {
    const order = this.orderBottomToTop();
    const i = order.indexOf(id);
    if (i < 0 || i >= order.length - 1) return;
    this.apply([B.moveLayer(id, i + 1)]);
  }
  /** 레이어를 한 칸 아래(뒤)로 — z-order 하강. 맨 아래면 무시. */
  lower(id) {
    const order = this.orderBottomToTop();
    const i = order.indexOf(id);
    if (i <= 0) return;
    this.apply([B.moveLayer(id, i - 1)]);
  }

  /** 라이브 모드 진입(쓰기 funnel을 데몬으로 전환). live.js가 호출. */
  setLive(link) {
    this.live = link;
  }

  /** 라이브 snapshot으로 editor 교체 + 렌더러 재배선 + 첫 프레임. */
  replaceEditor(editor) {
    this.editor = editor;
    this.renderer.editor = editor;
    this.renderer.resize();
    this._notify();
  }

  /** 원격(데몬 broadcast) 적용 후 화면·패널 갱신. */
  afterRemoteApply() {
    this.renderer.markDirty();
    this._notify();
  }

  /** Action 배열을 적용한다(단일 쓰기 funnel).
   *  로컬 모드: 즉시 wasm 적용. 라이브 모드: 데몬에만 보내고 적용은 broadcast 수신 시. */
  apply(actions) {
    if (this.live) {
      this.live.sendApply(actions);
      return { ok: true, deferred: true };
    }
    const res = JSON.parse(this.editor.apply_actions(JSON.stringify(actions)));
    if (!res.ok) {
      console.error("apply 실패:", res.issues);
      this._notify();
      return res;
    }
    this.renderer.markDirty();
    this._notify();
    return res;
  }

  undo() {
    if (this.live) {
      this.live.sendUndo();
      return;
    }
    if (this.editor.undo()) {
      this.renderer.markDirty();
      this._notify();
    }
  }
  redo() {
    if (this.live) {
      this.live.sendRedo();
      return;
    }
    if (this.editor.redo()) {
      this.renderer.markDirty();
      this._notify();
    }
  }

  /** 레이어 목록(파생 뷰, top-to-bottom). */
  layers() {
    const j = JSON.parse(this.editor.layers());
    return j.layers.slice().reverse(); // bottom-to-top → 표시용 top-to-bottom
  }
  canUndo() { return this.editor.can_undo(); }
  canRedo() { return this.editor.can_redo(); }
  docInfo() { return JSON.parse(this.editor.doc_info()); }

  _notify() {
    this.dispatchEvent(new CustomEvent("changed"));
  }
}
