// Lit Web Components — React 금지, 경량. 상태는 App(Editor)이 소유, 컴포넌트는 파생 뷰.

import { LitElement, html, css } from "lit";
import * as B from "./bridge.js";

const RGBA = (hex, alpha) => {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255, Math.round(alpha * 255)];
};

// ---------------- 툴바 ----------------
class DxToolbar extends LitElement {
  static properties = {
    app: { attribute: false },
    tool: { state: true },
    color: { state: true },
    alpha: { state: true },
    width: { state: true },
    _v: { state: true },
  };
  static styles = css`
    :host { display: flex; gap: 8px; align-items: center; padding: 8px 12px;
            background: #2a2a2a; border-bottom: 1px solid #000; flex-wrap: wrap; }
    button { background: #3a3a3a; color: #ddd; border: 1px solid #555; border-radius: 4px;
             padding: 6px 10px; cursor: pointer; font-size: 13px; }
    button.active { background: #4a7; color: #000; border-color: #4a7; }
    button:disabled { opacity: 0.4; cursor: default; }
    label { font-size: 12px; color: #aaa; display: flex; align-items: center; gap: 4px; }
    .sep { width: 1px; height: 22px; background: #555; margin: 0 4px; }
    input[type="color"] { width: 32px; height: 28px; border: none; background: none; padding: 0; }
  `;
  constructor() {
    super();
    this.tool = "rect"; this.color = "#4aa3ff"; this.alpha = 1; this.width = 4; this._v = 0;
  }
  connectedCallback() {
    super.connectedCallback();
    this._onChange = () => { this._v++; };
    this.app?.addEventListener("changed", this._onChange);
  }
  disconnectedCallback() { this.app?.removeEventListener("changed", this._onChange); super.disconnectedCallback(); }

  _pick(t) { this.tool = t; this.dispatchEvent(new CustomEvent("tool-changed", { detail: this._toolState(), bubbles: true, composed: true })); }
  _toolState() { return { tool: this.tool, rgba: RGBA(this.color, this.alpha), width: this.width }; }
  _emit() { this.dispatchEvent(new CustomEvent("tool-changed", { detail: this._toolState(), bubbles: true, composed: true })); }

  render() {
    const t = (id, label) => html`<button class=${this.tool === id ? "active" : ""} @click=${() => this._pick(id)}>${label}</button>`;
    return html`
      ${t("rect", "▭ 사각형")} ${t("ellipse", "◯ 원")} ${t("line", "／ 선")}
      <span class="sep"></span>
      <label>색 <input type="color" .value=${this.color}
        @input=${(e) => { this.color = e.target.value; this._emit(); }} /></label>
      <label>투명도 <input type="range" min="0" max="1" step="0.05" .value=${String(this.alpha)}
        @input=${(e) => { this.alpha = +e.target.value; this._emit(); }} /></label>
      <label>두께 <input type="range" min="1" max="30" step="1" .value=${String(this.width)}
        @input=${(e) => { this.width = +e.target.value; this._emit(); }} /></label>
      <span class="sep"></span>
      <button ?disabled=${!this.app?.canUndo()} @click=${() => this.app.undo()}>↶ 되돌리기</button>
      <button ?disabled=${!this.app?.canRedo()} @click=${() => this.app.redo()}>↷ 다시</button>
      <span class="sep"></span>
      <button @click=${() => this.dispatchEvent(new CustomEvent("export-png", { bubbles: true, composed: true }))}>🖼 PNG</button>
      <button @click=${() => this.dispatchEvent(new CustomEvent("save-dxpkg", { bubbles: true, composed: true }))}>💾 저장</button>
    `;
  }
}
customElements.define("dx-toolbar", DxToolbar);

// ---------------- 캔버스(드래그 그리기) ----------------
class DxCanvas extends LitElement {
  static properties = { app: { attribute: false }, toolState: { attribute: false } };
  static styles = css`
    :host { display: block; overflow: auto; background:
      repeating-conic-gradient(#2a2a2a 0% 25%, #333 0% 50%) 50% / 20px 20px; }
    .wrap { position: relative; margin: 24px; width: fit-content; }
    canvas { display: block; image-rendering: pixelated; box-shadow: 0 0 0 1px #000, 0 8px 30px rgba(0,0,0,0.5); }
    #overlay { position: absolute; left: 0; top: 0; pointer-events: none; }
  `;
  constructor() { super(); this._drag = null; }
  firstUpdated() {
    this.base = this.renderRoot.querySelector("#base");
    this.overlay = this.renderRoot.querySelector("#overlay");
    this.app.renderer.canvas = this.base; // Renderer가 이 캔버스에 그림
    this.app.renderer.resize();
    this._sizeOverlay();
    this.base.addEventListener("pointerdown", (e) => this._down(e));
    window.addEventListener("pointermove", (e) => this._move(e));
    window.addEventListener("pointerup", (e) => this._up(e));
  }
  _sizeOverlay() {
    this.overlay.width = this.base.width; this.overlay.height = this.base.height;
    this.overlay.style.width = this.base.style.width || this.base.width + "px";
    this.overlay.style.height = this.base.style.height || this.base.height + "px";
  }
  _coords(e) {
    const r = this.base.getBoundingClientRect();
    return {
      x: (e.clientX - r.left) * (this.base.width / r.width),
      y: (e.clientY - r.top) * (this.base.height / r.height),
    };
  }
  _down(e) { this._drag = { start: this._coords(e), cur: this._coords(e) }; this.base.setPointerCapture?.(e.pointerId); }
  _move(e) {
    if (!this._drag) return;
    this._drag.cur = this._coords(e);
    this._drawGhost();
  }
  _drawGhost() {
    const o = this.overlay.getContext("2d");
    o.clearRect(0, 0, this.overlay.width, this.overlay.height);
    const s = this.toolState; const [r, g, b, a] = s.rgba;
    o.strokeStyle = `rgba(${r},${g},${b},${a / 255})`;
    o.fillStyle = `rgba(${r},${g},${b},${(a / 255) * 0.5})`;
    o.lineWidth = 1;
    const { start, cur } = this._drag;
    if (s.tool === "rect") { o.fillRect(Math.min(start.x, cur.x), Math.min(start.y, cur.y), Math.abs(cur.x - start.x), Math.abs(cur.y - start.y)); }
    else if (s.tool === "ellipse") { o.beginPath(); o.ellipse((start.x + cur.x) / 2, (start.y + cur.y) / 2, Math.abs(cur.x - start.x) / 2, Math.abs(cur.y - start.y) / 2, 0, 0, 7); o.fill(); }
    else { o.lineWidth = s.width; o.beginPath(); o.moveTo(start.x, start.y); o.lineTo(cur.x, cur.y); o.stroke(); }
  }
  _up() {
    if (!this._drag) return;
    const o = this.overlay.getContext("2d");
    o.clearRect(0, 0, this.overlay.width, this.overlay.height);
    const { start, cur } = this._drag;
    this._drag = null;
    const s = this.toolState; const rgba = s.rgba;
    let shape, name;
    if (s.tool === "rect") {
      const w = Math.abs(cur.x - start.x), h = Math.abs(cur.y - start.y);
      if (w < 1 || h < 1) return; // degenerate 가드
      shape = B.rect(Math.min(start.x, cur.x), Math.min(start.y, cur.y), w, h, rgba); name = "사각형";
    } else if (s.tool === "ellipse") {
      const rx = Math.abs(cur.x - start.x) / 2, ry = Math.abs(cur.y - start.y) / 2;
      if (rx < 0.5 || ry < 0.5) return;
      shape = B.ellipse((start.x + cur.x) / 2, (start.y + cur.y) / 2, rx, ry, rgba); name = "원";
    } else {
      const dx = cur.x - start.x, dy = cur.y - start.y;
      if (Math.hypot(dx, dy) < 1) return;
      shape = B.line(start.x, start.y, cur.x, cur.y, s.width, rgba); name = "선";
    }
    this.app.apply([B.addPaintLayer(name, B.shapes([shape]))]);
  }
  render() {
    return html`<div class="wrap"><canvas id="base"></canvas><canvas id="overlay"></canvas></div>`;
  }
}
customElements.define("dx-canvas", DxCanvas);

// ---------------- 레이어 패널 ----------------
class DxLayerPanel extends LitElement {
  static properties = { app: { attribute: false }, _v: { state: true } };
  static styles = css`
    :host { display: block; width: 260px; background: #252525; border-left: 1px solid #000;
            overflow-y: auto; padding: 8px; }
    h3 { margin: 4px 8px 8px; font-size: 13px; color: #aaa; }
    .layer { background: #2e2e2e; border: 1px solid #444; border-radius: 5px; padding: 8px; margin-bottom: 6px; }
    .row { display: flex; align-items: center; gap: 6px; margin: 3px 0; font-size: 12px; }
    .name { flex: 1; font-weight: 600; }
    select, input { background: #3a3a3a; color: #ddd; border: 1px solid #555; border-radius: 3px; }
    .del { background: #633; color: #fbb; border: none; border-radius: 3px; cursor: pointer; padding: 2px 6px; }
    input[type="range"] { flex: 1; }
  `;
  constructor() { super(); this._v = 0; }
  connectedCallback() {
    super.connectedCallback();
    this._onChange = () => { this._v++; };
    this.app?.addEventListener("changed", this._onChange);
  }
  disconnectedCallback() { this.app?.removeEventListener("changed", this._onChange); super.disconnectedCallback(); }

  render() {
    const layers = this.app ? this.app.layers() : [];
    return html`
      <h3>레이어 (${layers.length})</h3>
      ${layers.length === 0 ? html`<div style="color:#666;padding:8px;font-size:12px">캔버스에 그려보세요</div>` : ""}
      ${layers.map((l) => html`
        <div class="layer">
          <div class="row">
            <input type="checkbox" .checked=${l.visible}
              @change=${(e) => this.app.apply([B.setProps(l.id, { visible: e.target.checked })])} />
            <span class="name">${l.name}</span>
            <button class="del" @click=${() => this.app.apply([B.deleteLayer(l.id)])}>🗑</button>
          </div>
          <div class="row">투명도
            <input type="range" min="0" max="1" step="0.01" .value=${String(l.opacity)}
              @change=${(e) => this.app.apply([B.setProps(l.id, { opacity: +e.target.value })])} />
          </div>
          <div class="row">블렌드
            <select .value=${l.blend}
              @change=${(e) => this.app.apply([B.setBlend(l.id, e.target.value)])}>
              <option value="normal">Normal</option>
              <option value="multiply">Multiply</option>
              <option value="screen">Screen</option>
            </select>
          </div>
        </div>`)}
    `;
  }
}
customElements.define("dx-layer-panel", DxLayerPanel);

// ---------------- 앱 셸 ----------------
class AppShell extends LitElement {
  static properties = { app: { attribute: false }, _tool: { state: true } };
  static styles = css`
    :host { display: grid; grid-template-rows: auto 1fr; grid-template-columns: 1fr auto;
            grid-template-areas: "toolbar toolbar" "canvas panel"; height: 100vh; }
    dx-toolbar { grid-area: toolbar; }
    dx-canvas { grid-area: canvas; }
    dx-layer-panel { grid-area: panel; }
  `;
  constructor() { super(); this._tool = { tool: "rect", rgba: [74, 163, 255, 255], width: 4 }; }
  render() {
    if (!this.app) return html`<div style="padding:40px">로딩 중…</div>`;
    return html`
      <dx-toolbar .app=${this.app}
        @tool-changed=${(e) => { this._tool = e.detail; }}
        @export-png=${() => this.dispatchEvent(new CustomEvent("export-png", { bubbles: true }))}
        @save-dxpkg=${() => this.dispatchEvent(new CustomEvent("save-dxpkg", { bubbles: true }))}></dx-toolbar>
      <dx-canvas .app=${this.app} .toolState=${this._tool}></dx-canvas>
      <dx-layer-panel .app=${this.app}></dx-layer-panel>
    `;
  }
}
customElements.define("app-shell", AppShell);
