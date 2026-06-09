// Lit Web Components — React 금지, 경량. 상태는 App(Editor)이 소유, 컴포넌트는 파생 뷰.
//
// 미학: Light "Precision Studio" — 라이트톤 + 높은 대비 + 큰 폰트로 가독성 우선.
// 정체성(모노스페이스 계측기)은 유지, 선택/액센트는 선명한 인디고. 토큰은 index.html :root 상속.

import { LitElement, html, css } from "lit";
import * as B from "./bridge.js";

const RGBA = (hex, alpha) => {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255, Math.round(alpha * 255)];
};

// 공용 컨트롤 스타일.
const controls = css`
  button {
    font-family: var(--sans); font-size: 12.5px; font-weight: 500;
    background: var(--surface-2); color: var(--fg);
    border: 1px solid var(--line-2); border-radius: var(--radius);
    height: var(--ctl-h); padding: 0 13px; cursor: pointer;
    display: inline-flex; align-items: center; gap: 6px;
    transition: background 0.12s, color 0.12s, border-color 0.12s, box-shadow 0.12s;
  }
  button:hover { background: var(--sunken); border-color: var(--fg-faint); }
  button.active {
    background: var(--accent); color: #fff; border-color: var(--accent-strong);
    box-shadow: 0 1px 3px var(--accent-soft);
  }
  button:disabled { opacity: 0.35; cursor: default; }
  input, select {
    font-family: var(--mono); font-size: var(--fz-mono); color: var(--fg);
    background: var(--surface-2); border: 1px solid var(--line-2); border-radius: var(--radius);
    height: var(--ctl-h); padding: 0 9px; outline: none;
    transition: border-color 0.12s, box-shadow 0.12s;
  }
  input:focus, select:focus {
    border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-soft);
  }
  input[type="range"] { padding: 0; accent-color: var(--accent); height: auto; }
`;

// ───────── 상단 액션 바 ─────────
class DxTopbar extends LitElement {
  static properties = { app: { attribute: false }, _v: { state: true } };
  static styles = [controls, css`
    :host {
      grid-area: topbar; display: flex; align-items: center; gap: 9px;
      height: 50px; padding: 0 16px;
      background: var(--surface); border-bottom: 1px solid var(--line);
    }
    .mark {
      font-family: var(--mono); font-weight: 700; font-size: 15px;
      letter-spacing: 0.5px; color: var(--fg); display: flex; align-items: center; gap: 8px;
    }
    .mark .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--accent); }
    .mark b { color: var(--accent); }
    .meta {
      font-family: var(--mono); font-size: 11.5px; letter-spacing: 0.3px;
      color: var(--fg-dim); padding: 4px 9px; background: var(--sunken);
      border-radius: 5px; margin-left: 8px;
    }
    .spacer { flex: 1; }
    .ico { width: 34px; padding: 0; justify-content: center; font-size: 16px; }
  `];
  constructor() { super(); this._v = 0; }
  connectedCallback() { super.connectedCallback(); this._onChange = () => { this._v++; }; this.app?.addEventListener("changed", this._onChange); }
  disconnectedCallback() { this.app?.removeEventListener("changed", this._onChange); super.disconnectedCallback(); }
  render() {
    const info = this.app?.docInfo?.() ?? {};
    return html`
      <span class="mark"><span class="dot"></span>Design<b>CLI</b></span>
      <span class="meta">${info.w ?? "··"} × ${info.h ?? "··"} · ${(info.depth ?? "").toUpperCase()}</span>
      <span class="spacer"></span>
      <button class="ico" title="undo" ?disabled=${!this.app?.canUndo()} @click=${() => this.app.undo()}>↶</button>
      <button class="ico" title="redo" ?disabled=${!this.app?.canRedo()} @click=${() => this.app.redo()}>↷</button>
      <button @click=${() => this.dispatchEvent(new CustomEvent("export-png", { bubbles: true, composed: true }))}>Export</button>
      <button @click=${() => this.dispatchEvent(new CustomEvent("save-dxpkg", { bubbles: true, composed: true }))}>Save</button>
    `;
  }
}
customElements.define("dx-topbar", DxTopbar);

// ───────── 플로팅 툴바 ─────────
class DxToolbar extends LitElement {
  static properties = { tool: { state: true }, color: { state: true }, alpha: { state: true }, width: { state: true } };
  static styles = [controls, css`
    :host {
      display: flex; gap: 7px; align-items: center;
      background: var(--surface); border: 1px solid var(--line-2); border-radius: 12px; padding: 8px;
      box-shadow: 0 8px 28px rgba(28,27,24,0.14), 0 2px 6px rgba(28,27,24,0.08);
    }
    .tools { display: flex; gap: 5px; }
    .tools button { width: 40px; height: 40px; padding: 0; justify-content: center; font-size: 18px; }
    .sep { width: 1px; height: 28px; background: var(--line); margin: 0 5px; }
    .swatch { width: 34px; height: 34px; border: 1px solid var(--line-2); border-radius: 7px;
              padding: 0; cursor: pointer; overflow: hidden; position: relative; }
    .swatch input { position: absolute; inset: -4px; width: calc(100% + 8px); height: calc(100% + 8px); border: none; padding: 0; cursor: pointer; background: none; }
    label { font-family: var(--mono); font-size: 12px; color: var(--fg-dim); display: flex; align-items: center; gap: 6px; }
    input[type="range"] { width: 88px; }
    .val { font-family: var(--mono); font-size: 12px; font-weight: 600; color: var(--teal); min-width: 26px; text-align: right; }
  `];
  constructor() { super(); this.tool = "select"; this.color = "#3a5bd9"; this.alpha = 1; this.width = 4; }
  _pick(t) { this.tool = t; this._emit(); }
  _toolState() { return { tool: this.tool, rgba: RGBA(this.color, this.alpha), width: this.width }; }
  _emit() { this.dispatchEvent(new CustomEvent("tool-changed", { detail: this._toolState(), bubbles: true, composed: true })); }
  firstUpdated() { this._emit(); }
  render() {
    const t = (id, label, key) => html`<button class=${this.tool === id ? "active" : ""}
      title="${id} (${key})" @click=${() => this._pick(id)}>${label}</button>`;
    const isDraw = this.tool !== "select";
    return html`
      <div class="tools">
        ${t("select", "⤡", "V")} ${t("rect", "▭", "R")} ${t("ellipse", "◯", "E")} ${t("line", "╱", "L")}
      </div>
      ${isDraw ? html`
        <span class="sep"></span>
        <span class="swatch" style="background:${this.color}">
          <input type="color" .value=${this.color} @input=${(e) => { this.color = e.target.value; this._emit(); }} /></span>
        <label>A<input type="range" min="0" max="1" step="0.05" .value=${String(this.alpha)}
          @input=${(e) => { this.alpha = +e.target.value; this._emit(); }} /><span class="val">${Math.round(this.alpha * 100)}</span></label>
        ${this.tool === "line" ? html`<label>W<input type="range" min="1" max="30" step="1" .value=${String(this.width)}
          @input=${(e) => { this.width = +e.target.value; this._emit(); }} /><span class="val">${this.width}</span></label>` : ""}
      ` : ""}
    `;
  }
}
customElements.define("dx-toolbar", DxToolbar);

// ───────── 캔버스 ─────────
class DxCanvas extends LitElement {
  static properties = { app: { attribute: false }, toolState: { attribute: false }, _v: { state: true } };
  static styles = css`
    :host {
      grid-area: canvas; position: relative; display: block; overflow: auto;
      background: var(--paper);
    }
    .floating { position: absolute; left: 50%; top: 20px; transform: translateX(-50%); z-index: 10;
                animation: drop 0.45s cubic-bezier(0.2,0.8,0.2,1) both; }
    @keyframes drop { from { opacity: 0; transform: translateX(-50%) translateY(-10px); } to { opacity: 1; transform: translateX(-50%) translateY(0); } }
    .wrap { position: relative; margin: 84px auto 52px; width: fit-content; animation: fade 0.5s ease 0.08s both; }
    @keyframes fade { from { opacity: 0; } to { opacity: 1; } }
    .frame { position: relative; padding: 0; border-radius: 4px;
             box-shadow: 0 0 0 1px var(--line-2), 0 18px 50px rgba(28,27,24,0.18); }
    canvas { display: block; image-rendering: pixelated; border-radius: 4px; }
    /* 투명 영역 체커(라이트). */
    #base { background: repeating-conic-gradient(#e9e7e2 0% 25%, #f3f1ec 0% 50%) 50% / 16px 16px; }
    #overlay { position: absolute; left: 0; top: 0; pointer-events: none; }
  `;
  constructor() { super(); this._drag = null; this._v = 0; }
  connectedCallback() {
    super.connectedCallback();
    this._onChange = () => { this._v++; this._drawOverlay(); };
    this.app?.addEventListener("changed", this._onChange);
  }
  disconnectedCallback() {
    this.app?.removeEventListener("changed", this._onChange);
    window.removeEventListener("pointermove", this._mv);
    window.removeEventListener("pointerup", this._up);
    super.disconnectedCallback();
  }
  firstUpdated() {
    this.base = this.renderRoot.querySelector("#base");
    this.overlay = this.renderRoot.querySelector("#overlay");
    this.app.renderer.canvas = this.base;
    this.app.renderer.resize();
    this._sizeOverlay();
    this.base.addEventListener("pointerdown", (e) => this._down(e));
    this._mv = (e) => this._move(e); this._up = (e) => this._end(e);
    window.addEventListener("pointermove", this._mv);
    window.addEventListener("pointerup", this._up);
  }
  updated() { this._sizeOverlay(); this._drawOverlay(); }
  _sizeOverlay() {
    if (!this.overlay || !this.base) return;
    if (this.overlay.width !== this.base.width) this.overlay.width = this.base.width;
    if (this.overlay.height !== this.base.height) this.overlay.height = this.base.height;
  }
  _coords(e) {
    const r = this.base.getBoundingClientRect();
    return { x: (e.clientX - r.left) * (this.base.width / r.width), y: (e.clientY - r.top) * (this.base.height / r.height) };
  }
  _down(e) {
    const p = this._coords(e);
    if (this.toolState?.tool === "select") {
      const hit = this.app.hitTest(p.x, p.y);
      this.app.select(hit);
      if (hit != null) {
        const layer = this.app.getSelected();
        this._drag = { mode: "move", start: p, baseOffset: layer?.offset ?? [0, 0], cur: p, id: hit };
        this.style.cursor = "grabbing";
      }
      this.base.setPointerCapture?.(e.pointerId);
      return;
    }
    this._drag = { mode: "draw", start: p, cur: p };
    this.base.setPointerCapture?.(e.pointerId);
  }
  _move(e) {
    if (!this._drag) return;
    this._drag.cur = this._coords(e);
    if (this._drag.mode === "move") this._drawOverlay(); else this._drawGhost();
  }
  _end() {
    if (!this._drag) return;
    const d = this._drag; this._drag = null; this.style.cursor = "";
    if (d.mode === "move") {
      const dx = Math.round(d.cur.x - d.start.x), dy = Math.round(d.cur.y - d.start.y);
      if (dx !== 0 || dy !== 0) this.app.apply([B.setOffset(d.id, [d.baseOffset[0] + dx, d.baseOffset[1] + dy])]);
      this._drawOverlay();
      return;
    }
    const o = this.overlay.getContext("2d");
    o.clearRect(0, 0, this.overlay.width, this.overlay.height);
    const { start, cur } = d; const s = this.toolState; const rgba = s.rgba;
    let shape, name;
    if (s.tool === "rect") {
      const w = Math.abs(cur.x - start.x), h = Math.abs(cur.y - start.y);
      if (w < 1 || h < 1) return;
      shape = B.rect(Math.min(start.x, cur.x), Math.min(start.y, cur.y), w, h, rgba); name = "rect";
    } else if (s.tool === "ellipse") {
      const rx = Math.abs(cur.x - start.x) / 2, ry = Math.abs(cur.y - start.y) / 2;
      if (rx < 0.5 || ry < 0.5) return;
      shape = B.ellipse((start.x + cur.x) / 2, (start.y + cur.y) / 2, rx, ry, rgba); name = "ellipse";
    } else {
      if (Math.hypot(cur.x - start.x, cur.y - start.y) < 1) return;
      shape = B.line(start.x, start.y, cur.x, cur.y, s.width, rgba); name = "line";
    }
    this.app.apply([B.addPaintLayer(name, B.shapes([shape]))]);
  }
  _drawGhost() {
    const o = this.overlay.getContext("2d");
    o.clearRect(0, 0, this.overlay.width, this.overlay.height);
    const s = this.toolState; const [r, g, b, a] = s.rgba;
    o.strokeStyle = `rgba(${r},${g},${b},${a / 255})`;
    o.fillStyle = `rgba(${r},${g},${b},${(a / 255) * 0.45})`;
    o.lineWidth = 1; o.setLineDash([4, 3]);
    const { start, cur } = this._drag;
    if (s.tool === "rect") { o.fillRect(Math.min(start.x, cur.x), Math.min(start.y, cur.y), Math.abs(cur.x - start.x), Math.abs(cur.y - start.y)); o.strokeRect(Math.min(start.x, cur.x), Math.min(start.y, cur.y), Math.abs(cur.x - start.x), Math.abs(cur.y - start.y)); }
    else if (s.tool === "ellipse") { o.beginPath(); o.ellipse((start.x + cur.x) / 2, (start.y + cur.y) / 2, Math.abs(cur.x - start.x) / 2, Math.abs(cur.y - start.y) / 2, 0, 0, 7); o.fill(); o.stroke(); }
    else { o.setLineDash([]); o.lineWidth = s.width; o.beginPath(); o.moveTo(start.x, start.y); o.lineTo(cur.x, cur.y); o.stroke(); }
    o.setLineDash([]);
  }
  // 셀렉션 박스 — 선명한 인디고(라이트 배경에서 또렷). 흰 테두리 핸들로 대비 보강.
  _drawOverlay() {
    if (!this.overlay) return;
    const o = this.overlay.getContext("2d");
    o.clearRect(0, 0, this.overlay.width, this.overlay.height);
    if (this._drag?.mode === "draw") { this._drawGhost(); return; }
    const sel = this.app.getSelected?.();
    if (!sel) return;
    const bounds = this.app.layerBounds(sel.id);
    if (!bounds) return;
    let [x, y, w, h] = bounds;
    if (this._drag?.mode === "move") { x += Math.round(this._drag.cur.x - this._drag.start.x); y += Math.round(this._drag.cur.y - this._drag.start.y); }
    // 채움 + 또렷한 2px 인디고 박스.
    o.fillStyle = "rgba(58,91,217,0.10)";
    o.fillRect(x, y, w, h);
    o.strokeStyle = "#3a5bd9"; o.lineWidth = 2;
    o.strokeRect(x + 1, y + 1, w - 2, h - 2);
    // 리사이즈 핸들(8): 미지원 → 흰 채움 + 인디고 테두리(비활성 룩, 대비 높음).
    const hs = 8;
    const pts = [[x, y], [x + w / 2, y], [x + w, y], [x, y + h / 2], [x + w, y + h / 2], [x, y + h], [x + w / 2, y + h], [x + w, y + h]];
    for (const [px, py] of pts) {
      o.fillStyle = "#ffffff"; o.fillRect(px - hs / 2, py - hs / 2, hs, hs);
      o.strokeStyle = "rgba(58,91,217,0.55)"; o.lineWidth = 1; o.strokeRect(px - hs / 2 + 0.5, py - hs / 2 + 0.5, hs - 1, hs - 1);
    }
  }
  render() {
    return html`
      <div class="floating"><dx-toolbar @tool-changed=${(e) => { this.toolState = e.detail; }}></dx-toolbar></div>
      <div class="wrap"><div class="frame">
        <canvas id="base"></canvas><canvas id="overlay"></canvas>
      </div></div>
    `;
  }
}
customElements.define("dx-canvas", DxCanvas);

// ───────── 레이어 패널 (좌측) ─────────
class DxLayerPanel extends LitElement {
  static styles = css`
    :host {
      grid-area: layers; display: flex; flex-direction: column; width: 248px;
      background: var(--surface); border-right: 1px solid var(--line); overflow: hidden;
    }
    .head {
      font-family: var(--mono); padding: 15px 15px 10px; font-size: 11.5px; font-weight: 600;
      letter-spacing: 1px; color: var(--fg-dim); display: flex; align-items: center; justify-content: space-between;
    }
    .head .count { color: var(--accent); margin-left: auto; margin-right: 8px; }
    .add {
      font-family: var(--sans); font-size: 15px; line-height: 1; background: var(--surface-2);
      color: var(--fg-dim); border: 1px solid var(--line-2); border-radius: 6px;
      width: 24px; height: 24px; cursor: pointer; padding: 0;
    }
    .add:hover { background: var(--accent); color: #fff; border-color: var(--accent-strong); }
    .menu {
      position: absolute; right: 12px; top: 42px; z-index: 20; background: var(--surface-2);
      border: 1px solid var(--line-2); border-radius: 8px; padding: 5px;
      box-shadow: 0 8px 24px rgba(28,27,24,0.18); display: flex; flex-direction: column; gap: 2px; min-width: 150px;
    }
    .menu button {
      font-family: var(--sans); font-size: 12.5px; text-align: left; background: none; border: none;
      color: var(--fg); padding: 8px 10px; border-radius: 5px; cursor: pointer; display: flex; align-items: center; gap: 8px;
    }
    .menu button:hover { background: var(--accent-soft); color: var(--accent-strong); }
    .menu .sw { width: 12px; height: 12px; border-radius: 3px; border: 1px solid var(--line-2); }
    .list { flex: 1; overflow-y: auto; padding: 2px 8px 10px; position: relative; }
    .row {
      display: flex; align-items: center; gap: 8px; padding: 9px 10px; border-radius: var(--radius);
      font-size: 13px; cursor: pointer; color: var(--fg-dim); position: relative;
      transition: background 0.1s, color 0.1s;
    }
    .row:hover { background: var(--sunken); color: var(--fg); }
    .row.sel { background: var(--accent-soft); color: var(--accent-strong); font-weight: 500; }
    .row.sel::before { content: ""; position: absolute; left: 0; top: 7px; bottom: 7px; width: 3px; background: var(--accent); border-radius: 3px; }
    .swatch { width: 11px; height: 11px; border-radius: 3px; flex: none; border: 1px solid var(--line-2); }
    .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .name input {
      font-family: var(--sans); font-size: 13px; width: 100%; background: var(--surface-2);
      color: var(--fg); border: 1px solid var(--accent); border-radius: 4px; padding: 2px 5px; outline: none;
    }
    .id { font-family: var(--mono); font-size: 11px; color: var(--fg-faint); }
    .ord { display: flex; flex-direction: column; gap: 1px; flex: none; }
    .ord button { font-size: 8px; line-height: 1; height: 9px; }
    .btn { opacity: 0; background: none; border: none; color: inherit; cursor: pointer; font-size: 13px; padding: 0 2px; flex: none; }
    .row:hover .btn, .row.sel .btn, .row:hover .ord button, .row.sel .ord button { opacity: 0.7; }
    .ord button { opacity: 0; background: none; border: none; color: inherit; cursor: pointer; padding: 0; }
    .btn:hover, .ord button:hover { opacity: 1 !important; color: var(--accent); }
    .empty { padding: 22px 15px; color: var(--fg-faint); font-family: var(--mono); font-size: 12px; line-height: 1.8; }
    .empty b { color: var(--accent); }
  `;
  static properties = { app: { attribute: false }, _v: { state: true }, _menu: { state: true }, _editing: { state: true } };
  constructor() { super(); this._v = 0; this._menu = false; this._editing = null; }
  connectedCallback() {
    super.connectedCallback();
    this._onChange = () => { this._v++; };
    this.app?.addEventListener("changed", this._onChange);
    // 메뉴 바깥 클릭 시 닫기(컴포넌트 밖 클릭 — shadow 경계라 composedPath로 판별).
    this._onDoc = (e) => { if (this._menu && !e.composedPath().includes(this)) this._menu = false; };
    document.addEventListener("click", this._onDoc);
  }
  disconnectedCallback() {
    this.app?.removeEventListener("changed", this._onChange);
    document.removeEventListener("click", this._onDoc);
    super.disconnectedCallback();
  }

  // 빈/단색 레이어 추가. color=null이면 투명.
  _addLayer(color) {
    this._menu = false;
    const src = color ? B.fill(color) : B.transparent();
    this.app.apply([B.addPaintLayer(color ? "fill" : "layer", src)]);
  }
  // PNG 파일 업로드 → base64 → 레이어. 문서 크기와 일치해야 엔진이 받는다.
  _addPng(e) {
    this._menu = false;
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      // data:image/png;base64,XXXX → 순수 base64만.
      const b64 = String(reader.result).split(",")[1] || "";
      const res = this.app.apply([B.addPaintLayer(file.name.replace(/\.[^.]+$/, ""), B.pngBase64(b64))]);
      if (res && res.ok === false) {
        alert("이미지 추가 실패: " + (res.issues?.[0]?.message || "문서 크기와 일치하는 8bit RGBA PNG만 됩니다"));
      }
    };
    reader.readAsDataURL(file);
    e.target.value = ""; // 같은 파일 재선택 허용
  }
  _rename(id, v) {
    this._editing = null;
    const name = v.trim();
    if (name) this.app.apply([B.setProps(id, { name })]);
  }

  render() {
    const layers = this.app ? this.app.layers() : [];
    const selId = this.app?.selectedId;
    return html`
      <div class="head">
        <span>LAYERS</span>
        <span class="count">${String(layers.length).padStart(2, "0")}</span>
        <button class="add" title="레이어 추가" @click=${() => { this._menu = !this._menu; }}>+</button>
      </div>
      <div class="list">
        ${this._menu ? html`
          <div class="menu" @click=${(e) => e.stopPropagation()}>
            <button @click=${() => this._addLayer(null)}><span class="sw" style="background:repeating-conic-gradient(#ccc 0% 25%,#fff 0% 50%) 50%/6px 6px"></span>빈 레이어</button>
            <button @click=${() => this._addLayer([58, 91, 217, 255])}><span class="sw" style="background:#3a5bd9"></span>단색 (인디고)</button>
            <button @click=${() => this._addLayer([255, 255, 255, 255])}><span class="sw" style="background:#fff"></span>단색 (흰색)</button>
            <button @click=${() => this.renderRoot.querySelector("#png").click()}>🖼 PNG 불러오기…</button>
            <input id="png" type="file" accept="image/png" style="display:none" @change=${(e) => this._addPng(e)} />
          </div>` : ""}
        ${layers.length === 0 ? html`<div class="empty">no layers yet.<br>draw with <b>R / E / L</b><br>or pipe from <b>dx</b> cli.</div>` : ""}
        ${layers.map((l) => html`
          <div class="row ${l.id === selId ? "sel" : ""}" @click=${() => this.app.select(l.id)}>
            <span class="ord">
              <button title="위로" @click=${(e) => { e.stopPropagation(); this.app.raise(l.id); }}>▲</button>
              <button title="아래로" @click=${(e) => { e.stopPropagation(); this.app.lower(l.id); }}>▼</button>
            </span>
            <span class="swatch"></span>
            <span class="name" @dblclick=${(e) => { e.stopPropagation(); this._editing = l.id; }}>
              ${this._editing === l.id
                ? html`<input .value=${l.name} autofocus
                    @click=${(e) => e.stopPropagation()}
                    @keydown=${(e) => { if (e.key === "Enter") this._rename(l.id, e.target.value); if (e.key === "Escape") this._editing = null; }}
                    @blur=${(e) => this._rename(l.id, e.target.value)} />`
                : l.name}
            </span>
            <span class="id">#${l.id}</span>
            <button class="btn" title="visibility"
              @click=${(e) => { e.stopPropagation(); this.app.apply([B.setProps(l.id, { visible: !l.visible })]); }}>${l.visible ? "●" : "○"}</button>
            <button class="btn" title="delete"
              @click=${(e) => { e.stopPropagation(); this.app.apply([B.deleteLayer(l.id)]); }}>✕</button>
          </div>`)}
      </div>
    `;
  }
}
customElements.define("dx-layer-panel", DxLayerPanel);

// ───────── 속성 패널 (우측) ─────────
class DxProps extends LitElement {
  static properties = { app: { attribute: false }, _v: { state: true } };
  static styles = [controls, css`
    :host {
      grid-area: props; display: block; width: 248px; background: var(--surface);
      border-left: 1px solid var(--line); overflow-y: auto;
    }
    .head { font-family: var(--mono); padding: 15px 16px 13px; font-size: 11.5px; font-weight: 600;
            letter-spacing: 1px; color: var(--fg-dim); border-bottom: 1px solid var(--line); }
    .head b { color: var(--accent); }
    .sec { padding: 15px 16px; border-bottom: 1px solid var(--line); }
    .sec-t { font-family: var(--mono); font-size: 10.5px; font-weight: 600; letter-spacing: 1px; color: var(--fg-faint); margin-bottom: 12px; }
    .xy { display: grid; grid-template-columns: 16px 1fr 16px 1fr; gap: 9px; align-items: center; }
    .xy label { font-family: var(--mono); font-size: 12px; font-weight: 600; color: var(--fg-dim); }
    .xy input { width: 100%; text-align: right; }
    .field { margin-bottom: 15px; }
    .field:last-child { margin-bottom: 0; }
    .field > label { display: flex; justify-content: space-between; font-family: var(--mono);
                     font-size: 11.5px; color: var(--fg-dim); margin-bottom: 7px; }
    .field > label .v { color: var(--teal); font-weight: 600; }
    .field input[type="range"], .field select { width: 100%; }
    .chk { display: flex; align-items: center; gap: 9px; font-family: var(--mono); font-size: 12.5px; color: var(--fg-dim); cursor: pointer; }
    .chk input { width: 16px; height: 16px; accent-color: var(--accent); }
    .empty { padding: 26px 16px; font-family: var(--mono); font-size: 12px; color: var(--fg-faint); line-height: 1.8; }
    .note { padding: 13px 16px; font-family: var(--mono); font-size: 11px; color: var(--fg-faint);
            line-height: 1.7; background: var(--sunken); }
    .note b { color: var(--fg-dim); }
  `];
  constructor() { super(); this._v = 0; }
  connectedCallback() { super.connectedCallback(); this._onChange = () => { this._v++; }; this.app?.addEventListener("changed", this._onChange); }
  disconnectedCallback() { this.app?.removeEventListener("changed", this._onChange); super.disconnectedCallback(); }
  _set(patch) { this.app.apply([B.setProps(this.app.selectedId, patch)]); }
  render() {
    const l = this.app?.getSelected?.();
    if (!l) return html`<div class="head">INSPECT</div><div class="empty">// nothing selected<br>// pick the ⤡ tool,<br>// click a layer.</div>`;
    const [ox, oy] = l.offset ?? [0, 0];
    const commit = (which, v) => {
      const nx = which === "x" ? (+v | 0) : ox, ny = which === "y" ? (+v | 0) : oy;
      this.app.apply([B.setOffset(l.id, [nx, ny])]);
    };
    return html`
      <div class="head">INSPECT · <b>${l.name}</b></div>
      <div class="sec">
        <div class="sec-t">TRANSFORM</div>
        <div class="xy">
          <label>X</label><input type="number" .value=${String(ox)} @change=${(e) => commit("x", e.target.value)} />
          <label>Y</label><input type="number" .value=${String(oy)} @change=${(e) => commit("y", e.target.value)} />
        </div>
      </div>
      <div class="sec">
        <div class="sec-t">APPEARANCE</div>
        <div class="field">
          <label>OPACITY <span class="v">${Math.round(l.opacity * 100)}%</span></label>
          <input type="range" min="0" max="1" step="0.01" .value=${String(l.opacity)}
            @input=${(e) => this._set({ opacity: +e.target.value })} />
        </div>
        <div class="field">
          <label>BLEND</label>
          <select .value=${l.blend} @change=${(e) => this.app.apply([B.setBlend(l.id, e.target.value)])}>
            <option value="normal">Normal</option>
            <option value="multiply">Multiply</option>
            <option value="screen">Screen</option>
          </select>
        </div>
        <div class="field">
          <label class="chk"><input type="checkbox" .checked=${l.visible}
            @change=${(e) => this._set({ visible: e.target.checked })} /> VISIBLE</label>
        </div>
      </div>
      <div class="note">// resize / rotate: <b>not yet</b><br>// raster engine — move only<br>// (handles shown disabled)</div>
    `;
  }
}
customElements.define("dx-props", DxProps);

// ───────── 앱 셸 ─────────
class AppShell extends LitElement {
  static properties = { app: { attribute: false } };
  static styles = css`
    :host {
      display: grid; height: 100vh;
      grid-template-rows: 50px 1fr;
      grid-template-columns: auto 1fr auto;
      grid-template-areas: "topbar topbar topbar" "layers canvas props";
      background: var(--paper);
    }
  `;
  connectedCallback() {
    super.connectedCallback();
    this._key = (e) => {
      if (e.target.tagName === "INPUT" || e.target.tagName === "SELECT") return;
      const map = { v: "select", r: "rect", e: "ellipse", l: "line" };
      const t = map[e.key.toLowerCase()];
      const tb = this.renderRoot?.querySelector("dx-canvas")?.renderRoot?.querySelector("dx-toolbar");
      if (t && tb) tb._pick(t);
    };
    window.addEventListener("keydown", this._key);
  }
  disconnectedCallback() { window.removeEventListener("keydown", this._key); super.disconnectedCallback(); }
  render() {
    if (!this.app) return html`<div style="padding:40px;font-family:var(--mono);color:var(--fg-faint)">// booting…</div>`;
    return html`
      <dx-topbar .app=${this.app}
        @export-png=${() => this.dispatchEvent(new CustomEvent("export-png", { bubbles: true }))}
        @save-dxpkg=${() => this.dispatchEvent(new CustomEvent("save-dxpkg", { bubbles: true }))}></dx-topbar>
      <dx-layer-panel .app=${this.app}></dx-layer-panel>
      <dx-canvas .app=${this.app}></dx-canvas>
      <dx-props .app=${this.app}></dx-props>
    `;
  }
}
customElements.define("app-shell", AppShell);
