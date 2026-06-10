// Lit Web Components — React 금지, 경량. 상태는 App(Editor)이 소유, 컴포넌트는 파생 뷰.
//
// 디자인: Figma풍 다크/라이트(토큰은 index.html). ★이모지 금지 — 아이콘 전부 인라인 SVG★
// 좌 레이어 패널 / 중앙 캔버스(줌·팬) / 우 Design 패널 / 상단 툴바(도구·줌·테마).

import { LitElement, html, css, svg, nothing } from "lit";
import * as B from "./bridge.js";

const RGBA = (hex, alpha) => {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255, Math.round(alpha * 255)];
};
const HEX = (rgba) =>
  "#" + rgba.slice(0, 3).map((v) => v.toString(16).padStart(2, "0")).join("");

// ───────── SVG 아이콘 (16px stroke, currentColor) ─────────
const P = {
  cursor: svg`<path d="M4.5 2.5l9 5.5-4 1.1 2.2 4.4-1.8.9-2.2-4.4-3.2 2.9z" fill="currentColor" stroke="none"/>`,
  square: svg`<rect x="2.5" y="2.5" width="11" height="11" rx="1"/>`,
  squareFill: svg`<rect x="2.5" y="2.5" width="11" height="11" rx="1" fill="currentColor" stroke="none"/>`,
  circle: svg`<circle cx="8" cy="8" r="5.5"/>`,
  circleFill: svg`<circle cx="8" cy="8" r="5.5" fill="currentColor" stroke="none"/>`,
  rounded: svg`<rect x="2.5" y="2.5" width="11" height="11" rx="4"/>`,
  line: svg`<path d="M3 13L13 3"/>`,
  text: svg`<path d="M3.5 4.5V3h9v1.5M8 3v10M6 13h4"/>`,
  image: svg`<rect x="2.5" y="2.5" width="11" height="11" rx="1"/><circle cx="6" cy="6" r="1.2" fill="currentColor" stroke="none"/><path d="M3 12l3.5-3.5 2 2L11 8l2.5 2.5"/>`,
  dropper: svg`<path d="M9.5 4.5l2-2a1.4 1.4 0 012 2l-2 2M8.5 5.5l-5 5L3 13l2.5-.5 5-5z"/>`,
  plus: svg`<path d="M8 3v10M3 8h10"/>`,
  chevDown: svg`<path d="M4 6.5l4 4 4-4"/>`,
  chevRight: svg`<path d="M6.5 4l4 4-4 4"/>`,
  chevLeft: svg`<path d="M9.5 4l-4 4 4 4"/>`,
  chevUpS: svg`<path d="M4 9.5l4-4 4 4"/>`,
  chevDownS: svg`<path d="M4 6.5l4 4 4-4"/>`,
  eye: svg`<path d="M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z"/><circle cx="8" cy="8" r="2"/>`,
  eyeOff: svg`<path d="M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z"/><path d="M3 13L13 3"/>`,
  trash: svg`<path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.5 8.5h6l.5-8.5M6.5 7v4M9.5 7v4"/>`,
  undo: svg`<path d="M6 3.5L3 6.5l3 3"/><path d="M3 6.5h6.5a3.5 3.5 0 010 7H7"/>`,
  redo: svg`<path d="M10 3.5l3 3-3 3"/><path d="M13 6.5H6.5a3.5 3.5 0 000 7H9"/>`,
  sun: svg`<circle cx="8" cy="8" r="3"/><path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4"/>`,
  moon: svg`<path d="M13.5 9.5A6 6 0 016.5 2.5a6 6 0 107 7z"/>`,
  export: svg`<path d="M8 10V2.5M5 5l3-3 3 3M3 9.5V13a.9.9 0 00 1 1h8a.9.9 0 001-1V9.5"/>`,
  save: svg`<path d="M3 3h8.5L13 4.5V13H3z"/><path d="M5 3v3.5h5.5V3M5 13V9h6v4"/>`,
  dup: svg`<rect x="5.5" y="5.5" width="8" height="8" rx="1"/><path d="M10.5 2.5h-8v8"/>`,
  alignL: svg`<path d="M2.5 2v12"/><rect x="4.5" y="4" width="8" height="3" rx=".5"/><rect x="4.5" y="9" width="5" height="3" rx=".5"/>`,
  alignCH: svg`<path d="M8 2v12"/><rect x="3" y="4" width="10" height="3" rx=".5"/><rect x="5" y="9" width="6" height="3" rx=".5"/>`,
  alignR: svg`<path d="M13.5 2v12"/><rect x="3" y="4" width="8" height="3" rx=".5"/><rect x="6" y="9" width="5" height="3" rx=".5"/>`,
  alignT: svg`<path d="M2 2.5h12"/><rect x="4" y="4.5" width="3" height="8" rx=".5"/><rect x="9" y="4.5" width="3" height="5" rx=".5"/>`,
  alignCV: svg`<path d="M2 8h12"/><rect x="4" y="3" width="3" height="10" rx=".5"/><rect x="9" y="5" width="3" height="6" rx=".5"/>`,
  alignB: svg`<path d="M2 13.5h12"/><rect x="4" y="3.5" width="3" height="8" rx=".5"/><rect x="9" y="6.5" width="3" height="5" rx=".5"/>`,
  pencil: svg`<path d="M3 11.5l-.5 2 2-.5 7.8-7.8a1.4 1.4 0 00-2-2z"/><path d="M9.3 4.2l2.5 2.5"/>`,
  frame: svg`<path d="M4.5 1.5v13M11.5 1.5v13M1.5 4.5h13M1.5 11.5h13"/>`,
  folder: svg`<path d="M1.5 4.5a1 1 0 011-1h3.2l1.4 1.8h6.4a1 1 0 011 1v5.7a1 1 0 01-1 1h-11a1 1 0 01-1-1z"/>`,
  download: svg`<path d="M8 2.5V10M5 7.5l3 3 3-3M3 13h10"/>`,
  play: svg`<path d="M5 3.5l7 4.5-7 4.5z" fill="currentColor" stroke="none"/>`,
  share: svg`<circle cx="4" cy="8" r="2"/><circle cx="12" cy="4" r="2"/><circle cx="12" cy="12" r="2"/><path d="M5.8 7.1l4.4-2.2M5.8 8.9l4.4 2.2"/>`,
};
const icon = (name, size = 15) => svg`
  <svg viewBox="0 0 16 16" width=${size} height=${size} fill="none"
    stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"
    style="display:block">${P[name]}</svg>`;

const SHAPES = [
  { id: "rect", ic: "squareFill", label: "사각형", key: "R" },
  { id: "ellipse", ic: "circleFill", label: "타원", key: "E" },
  { id: "stroke-rect", ic: "square", label: "테두리 사각형" },
  { id: "stroke-ellipse", ic: "circle", label: "테두리 타원" },
  { id: "rounded-rect", ic: "rounded", label: "둥근 사각형" },
];
const isShapeTool = (t) => SHAPES.some((s) => s.id === t);
const needsWidth = (t) => t === "line" || t === "stroke-rect" || t === "stroke-ellipse" || t === "brush";

// 공용 컨트롤 스타일(Figma풍: 28px 컨트롤, 보더리스 input, 블루 포커스).
const controls = css`
  button {
    font: inherit; background: none; color: var(--fg-2);
    border: none; border-radius: var(--radius); height: var(--ctl-h);
    padding: 0 8px; cursor: pointer; display: inline-flex; align-items: center; gap: 6px;
  }
  button:hover { background: var(--bg-hover); color: var(--fg); }
  button.active { background: var(--accent); color: #fff; }
  button:disabled { opacity: 0.35; cursor: default; background: none; }
  input, select {
    font: inherit; color: var(--fg); background: var(--bg-elev);
    border: 1px solid transparent; border-radius: var(--radius);
    height: var(--ctl-h); padding: 0 7px; outline: none;
  }
  input:hover, select:hover { border-color: var(--line); }
  input:focus, select:focus { border-color: var(--accent); }
  input[type="range"] { padding: 0; accent-color: var(--accent); height: auto; background: none; border: none; }
  input[type="checkbox"] { width: 14px; height: 14px; accent-color: var(--accent); }
`;

// ───────── 플로팅 툴바 ─────────
class DxTopbar extends LitElement {
  static properties = {
    app: { attribute: false }, zoom: { attribute: false }, theme: { attribute: false },
    tool: { state: true }, color: { state: true }, alpha: { state: true },
    width: { state: true }, radius: { state: true }, fontSize: { state: true },
    _shape: { state: true }, _menu: { state: true }, _exportMenu: { state: true }, _v: { state: true },
  };
  static styles = [controls, css`
    :host {
      display: contents;
    }
    .corner {
      position: fixed; right: 18px; top: 18px; z-index: 82;
      display: flex; align-items: center; gap: 2px;
      padding: 6px; background: var(--bg-panel);
      border: 1px solid var(--line); border-radius: 10px;
      box-shadow: 0 12px 34px rgba(0, 0, 0, 0.18);
    }
    .tools {
      position: fixed; left: 50%; bottom: 22px; z-index: 80;
      transform: translateX(-50%);
      display: flex; gap: 2px; align-items: center;
      padding: 7px 8px; background: var(--bg-panel);
      border: 1px solid var(--line); border-radius: 10px;
      box-shadow: 0 12px 34px rgba(0, 0, 0, 0.34);
    }
    .tools button { width: 34px; height: 34px; padding: 0; justify-content: center; }
    .tools button.dd { width: 44px; gap: 1px; }
    .sep { width: 1px; height: 20px; background: var(--line); margin: 0 7px; }
    .menu {
      position: absolute; left: 42px; bottom: 45px; z-index: 50; background: var(--bg-panel);
      border: 1px solid var(--line); border-radius: 9px; padding: 5px; min-width: 170px;
      box-shadow: var(--shadow-menu); display: flex; flex-direction: column; gap: 1px;
    }
    .menu button { width: 100%; justify-content: flex-start; height: 30px; color: var(--fg); }
    .menu button:hover { background: var(--accent); color: #fff; }
    .menu .key { margin-left: auto; color: inherit; opacity: 0.5; font-size: 10px; }
    .export-menu {
      position: fixed; right: 18px; top: 58px; z-index: 90; background: var(--bg-panel);
      border: 1px solid var(--line); border-radius: 9px; padding: 5px; min-width: 190px;
      box-shadow: var(--shadow-menu); display: flex; flex-direction: column; gap: 1px;
    }
    .export-menu button { width: 100%; justify-content: flex-start; height: 30px; color: var(--fg); }
    .export-menu button:hover { background: var(--accent); color: #10232c; }
    .export-menu .hr { height: 1px; background: var(--line-soft); margin: 4px 3px; }
    .opts {
      position: fixed; left: 50%; bottom: 84px; z-index: 79;
      transform: translateX(-50%);
      display: flex; align-items: center; gap: 8px;
      padding: 7px 9px; background: var(--bg-panel);
      border: 1px solid var(--line); border-radius: 9px;
      box-shadow: 0 10px 28px rgba(0, 0, 0, 0.28);
    }
    .swatch { width: 24px; height: 24px; border: 1px solid var(--line); border-radius: 5px;
              padding: 0; cursor: pointer; overflow: hidden; position: relative; flex: none; }
    .swatch input { position: absolute; inset: -4px; width: 36px; height: 36px; border: none; padding: 0; cursor: pointer; }
    .opts label { display: flex; align-items: center; gap: 5px; color: var(--fg-3); font-size: 10.5px; }
    .opts input[type="range"] { width: 64px; }
    .opts .num { width: 44px; text-align: right; }
    .zoom { display: flex; align-items: center; gap: 0; }
    .zoom .pct { min-width: 48px; text-align: center; color: var(--fg-2); cursor: pointer; }
    .ico { width: 30px; padding: 0; justify-content: center; }
  `];
  constructor() {
    super();
    this.tool = "select"; this.color = "#0d99ff"; this.alpha = 1;
    this.width = 4; this.radius = 12; this.fontSize = 32;
    this._shape = "rect"; this._menu = false; this._exportMenu = false; this._v = 0;
    this._returnTool = "select";
    this.zoom = 1; this.theme = "dark";
  }
  connectedCallback() {
    super.connectedCallback();
    this._onChange = () => { this._v++; };
    this.app?.addEventListener("changed", this._onChange);
    this._onDoc = (e) => {
      if ((this._menu || this._exportMenu) && !e.composedPath().includes(this)) {
        this._menu = false;
        this._exportMenu = false;
      }
    };
    document.addEventListener("click", this._onDoc);
  }
  disconnectedCallback() {
    this.app?.removeEventListener("changed", this._onChange);
    document.removeEventListener("click", this._onDoc);
    super.disconnectedCallback();
  }
  /** 외부(단축키/스포이드)에서 도구·색 설정. */
  setTool(t) { this._pick(t); }
  finishEyedrop() { this._pick(this._returnTool || "select"); }
  setColor(rgba) { this.color = HEX(rgba); this._emit(); }
  _pick(t) {
    if (t === "eyedrop") this._returnTool = this.tool === "eyedrop" ? this._returnTool : this.tool;
    this.tool = t;
    if (isShapeTool(t)) this._shape = t;
    this._menu = false;
    this._emit();
  }
  _toolState() {
    return {
      tool: this.tool, rgba: RGBA(this.color, this.alpha),
      width: this.width, radius: this.radius, size: this.fontSize,
    };
  }
  _emit() { this.dispatchEvent(new CustomEvent("tool-changed", { detail: this._toolState(), bubbles: true, composed: true })); }
  _zoomCmd(action) { this.dispatchEvent(new CustomEvent("zoom-cmd", { detail: action, bubbles: true, composed: true })); }
  firstUpdated() { this._emit(); }
  _addPng(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const b64 = String(reader.result).split(",")[1] || "";
      this.app?.apply([
        B.addPaintLayer(file.name.replace(/\.[^.]+$/, ""), B.pngBase64(b64), { bind: "img" }),
        B.setProps("img", { meta: JSON.stringify({ type: "image" }) }),
      ]);
    };
    reader.readAsDataURL(file);
    e.target.value = "";
  }
  render() {
    const cur = SHAPES.find((s) => s.id === this._shape) ?? SHAPES[0];
    const isDraw = this.tool !== "select" && this.tool !== "eyedrop" && this.tool !== "frame";
    const frames = this.app?.frames?.() ?? [];
    const t = (id, ic, title) => html`<button class=${this.tool === id ? "active" : ""}
      title=${title} @click=${() => this._pick(id)}>${icon(ic)}</button>`;
    return html`
      <div class="tools">
        ${t("select", "cursor", "선택/이동 (V)")}
        <button class="dd ${isShapeTool(this.tool) ? "active" : ""}" title="도형 (${cur.label})"
          @click=${() => { this._menu = !this._menu; }}>${icon(cur.ic)}${icon("chevDown", 9)}</button>
        ${t("line", "line", "선 (L)")}
        ${t("brush", "pencil", "브러시 (B)")}
        ${t("frame", "frame", "프레임 (F)")}
        ${t("text", "text", "텍스트 (T)")}
        <button title="이미지(PNG) 레이어" @click=${() => this.renderRoot.querySelector("#png").click()}>${icon("image")}</button>
        <input id="png" type="file" accept="image/png" style="display:none" @change=${(e) => this._addPng(e)} />
        ${this._menu ? html`
          <div class="menu">
            ${SHAPES.map((s) => html`
              <button @click=${() => this._pick(s.id)}>${icon(s.ic)}${s.label}
                ${s.key ? html`<span class="key">${s.key}</span>` : nothing}</button>`)}
          </div>` : nothing}
      </div>
      ${isDraw ? html`
        <span class="sep"></span>
        <div class="opts">
          <span class="swatch" style="background:${this.color}">
            <input type="color" .value=${this.color} @input=${(e) => { this.color = e.target.value; this._emit(); }} /></span>
          <button title="스포이드" @click=${() => this._pick("eyedrop")}>${icon("dropper", 13)}</button>
          <label>A<input type="range" min="0" max="1" step="0.05" .value=${String(this.alpha)}
            @input=${(e) => { this.alpha = +e.target.value; this._emit(); }} /></label>
          ${needsWidth(this.tool) ? html`<label>W<input class="num" type="number" min="1" max="100" .value=${String(this.width)}
            @change=${(e) => { this.width = +e.target.value || 1; this._emit(); }} /></label>` : nothing}
          ${this.tool === "rounded-rect" ? html`<label>R<input class="num" type="number" min="0" max="200" .value=${String(this.radius)}
            @change=${(e) => { this.radius = +e.target.value || 0; this._emit(); }} /></label>` : nothing}
          ${this.tool === "text" ? html`<label>크기<input class="num" type="number" min="6" max="400" .value=${String(this.fontSize)}
            @change=${(e) => { this.fontSize = +e.target.value || 12; this._emit(); }} /></label>` : nothing}
        </div>` : nothing}
      <div class="corner">
        <button class="ico" title="undo (Cmd+Z)" ?disabled=${!this.app?.canUndo()} @click=${() => this.app.undo()}>${icon("undo")}</button>
        <button class="ico" title="redo (Cmd+Shift+Z)" ?disabled=${!this.app?.canRedo()} @click=${() => this.app.redo()}>${icon("redo")}</button>
        <span class="sep"></span>
        <div class="zoom">
          <button class="ico" title="축소" @click=${() => this._zoomCmd("out")}>−</button>
          <span class="pct" title="100% (Shift+0) / 맞춤 (Shift+1)"
            @click=${() => this._zoomCmd("reset")}>${Math.round(this.zoom * 100)}%</span>
          <button class="ico" title="확대" @click=${() => this._zoomCmd("in")}>+</button>
        </div>
        <span class="sep"></span>
        <button title="내보내기" @click=${(e) => { e.stopPropagation(); this._exportMenu = !this._exportMenu; }}>
          ${icon("export")}Export${icon("chevDown", 9)}
        </button>
      </div>
      ${this._exportMenu ? html`
        <div class="export-menu">
          <button @click=${() => { this._exportMenu = false; this.dispatchEvent(new CustomEvent("export-png", { bubbles: true, composed: true })); }}>
            ${icon("export", 13)}전체 PNG
          </button>
          ${frames.length ? html`
            <div class="hr"></div>
            ${frames.map((f) => html`
              <button @click=${() => { this._exportMenu = false; this.app.exportFrame(f); }}>
                ${icon("frame", 13)}${f.name} PNG
              </button>`)}
          ` : nothing}
          <div class="hr"></div>
          <button @click=${() => { this._exportMenu = false; this.dispatchEvent(new CustomEvent("save-dxpkg", { bubbles: true, composed: true })); }}>
            ${icon("save", 13)}.dxpkg 저장
          </button>
        </div>` : nothing}
    `;
  }
}
customElements.define("dx-topbar", DxTopbar);

// ───────── 캔버스 (줌/팬 + 선택/이동/리사이즈/회전 + 그리기 + 텍스트 + 스포이드) ─────────
const HANDLE = 8; // 핸들 화면 px
class DxCanvas extends LitElement {
  static properties = {
    app: { attribute: false },
    toolState: { attribute: false },
    _v: { state: true },
    _text: { state: true },
    _ctx: { state: true },
    _bgMode: { state: true },
  };
  static styles = css`
    :host {
      grid-area: canvas; position: relative; display: block; overflow: hidden; background: var(--bg-canvas);
    }
    :host([data-bg="dot"]) {
      background: var(--bg-canvas);
      background-image: radial-gradient(rgba(106,116,128,0.48) 1px, transparent 1px);
      background-size: 24px 24px;
    }
    :host([data-bg="solid"]) { background-image: none; background: var(--bg-canvas); }
    .wrap { position: absolute; inset: 0; }
    canvas { display: block; position: absolute; left: 0; top: 0; transform-origin: 0 0; }
    #base { box-shadow: 0 0 0 1px var(--line), 0 10px 34px rgba(0,0,0,0.32); }
    #overlay { position: absolute; left: 0; top: 0; pointer-events: none; }
    textarea.txt {
      position: absolute; z-index: 20; background: transparent; color: var(--fg);
      border: 1.5px solid var(--accent); border-radius: 2px; outline: none; resize: none;
      font-family: "Pretendard", "Inter", sans-serif; line-height: 1.25; padding: 0; overflow: hidden;
      min-width: 40px; min-height: 1em; white-space: pre;
    }
    .ctx {
      position: absolute; z-index: 120; min-width: 168px; padding: 5px;
      background: var(--bg-panel); border: 1px solid var(--line); border-radius: 9px;
      box-shadow: var(--shadow-menu); display: flex; flex-direction: column; gap: 1px;
    }
    .ctx button {
      height: 30px; border: none; background: none; color: var(--fg);
      border-radius: 6px; padding: 0 8px; font: inherit; text-align: left;
      display: flex; align-items: center; gap: 7px; cursor: pointer;
    }
    .ctx button.active { background: var(--accent-soft); color: var(--fg); }
    .ctx button:hover { background: var(--accent); color: #10232c; }
    .ctx .hr { height: 1px; background: var(--line-soft); margin: 4px 3px; }
  `;
  constructor() {
    super();
    this._drag = null; this._hoverId = null; this._ctx = null; this._v = 0; this._zoom = 1; this._origin = { x: 0, y: 0 };
    this._space = false; this._text = null;
    this._bgMode = localStorage.getItem("dx.canvas.bg") || "dot";
  }
  connectedCallback() {
    super.connectedCallback();
    this._onChange = () => { this._v++; this._drawOverlay(); };
    this.app?.addEventListener("changed", this._onChange);
    this._mv = (e) => this._move(e); this._up = (e) => this._end(e);
    window.addEventListener("pointermove", this._mv);
    window.addEventListener("pointerup", this._up);
    this._kd = (e) => { if (e.code === "Space" && !this._isTyping(e)) { this._space = true; this.style.cursor = "grab"; e.preventDefault(); } };
    this._ku = (e) => { if (e.code === "Space") { this._space = false; this.style.cursor = ""; } };
    window.addEventListener("keydown", this._kd);
    window.addEventListener("keyup", this._ku);
    this._ctxDoc = (e) => {
      if (!this._ctx) return;
      const menu = this.renderRoot.querySelector(".ctx");
      if (menu && e.composedPath().includes(menu)) return;
      this._ctx = null;
    };
    document.addEventListener("pointerdown", this._ctxDoc);
  }
  disconnectedCallback() {
    this.app?.removeEventListener("changed", this._onChange);
    window.removeEventListener("pointermove", this._mv);
    window.removeEventListener("pointerup", this._up);
    window.removeEventListener("keydown", this._kd);
    window.removeEventListener("keyup", this._ku);
    document.removeEventListener("pointerdown", this._ctxDoc);
    super.disconnectedCallback();
  }
  _isTyping(e) {
    const t = e.composedPath?.()[0] ?? e.target;
    return t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT");
  }
  firstUpdated() {
    this.base = this.renderRoot.querySelector("#base");
    this.overlay = this.renderRoot.querySelector("#overlay");
    this.app.renderer.canvas = this.base;
    this._applyZoom();
    this.base.addEventListener("pointerdown", (e) => this._down(e));
    this.base.addEventListener("dblclick", (e) => this._dblclick(e));
    this.addEventListener("contextmenu", (e) => this._context(e));
    this.addEventListener("pointerleave", () => {
      if (this._hoverId != null) {
        this._hoverId = null;
        this._drawOverlay();
      }
    });
    this.addEventListener("wheel", (e) => this._wheel(e), { passive: false });
    this._ro = new ResizeObserver(() => this._applyZoom());
    this._ro.observe(this);
    this._applyBg();
  }

  _applyBg() {
    this.dataset.bg = this._bgMode;
  }

  _setBgMode(mode) {
    this._bgMode = mode;
    localStorage.setItem("dx.canvas.bg", mode);
    this._applyBg();
  }

  /** 텍스트 입력 오버레이 열기. 클릭 직후 캔버스의 포커스 스틸을 피해 지연 포커스. */
  _openText(state) {
    this._text = { born: performance.now(), ...state };
    // 기존 텍스트 편집이면 원본 레이어를 화면에서만 제외(문서·undo·동기화 무오염)
    // → 편집 박스 밑에 원본이 비치는 이중 표시 제거.
    if (state.editId != null) {
      this.app.renderer.excludeId = state.editId;
      this.app.renderer.markDirty();
    }
    this._drawOverlay(); // 편집 중엔 셀렉션 크롬 숨김.
    this.updateComplete.then(() => {
      const ta = this.renderRoot.querySelector("textarea.txt");
      if (!ta) return;
      // pointerup/click 시퀀스가 끝난 뒤 포커스(즉시 blur 방지).
      setTimeout(() => { ta.focus(); ta.select?.(); }, 0);
    });
  }

  /** 텍스트 작업 종료 공통 처리 — 화면 제외 해제 + 도구 select 복귀. */
  _finishText() {
    this._textDone = performance.now();
    if (this.app.renderer.excludeId != null) {
      this.app.renderer.excludeId = null;
      this.app.renderer.markDirty();
    }
    // Figma처럼 텍스트 작업이 끝나면 선택 도구로 복귀.
    this.dispatchEvent(new CustomEvent("text-finished", { bubbles: true, composed: true }));
    this._drawOverlay();
  }

  /** select 도구 더블클릭: 텍스트 레이어(meta.type==text)면 인라인 편집. */
  _dblclick(e) {
    try {
      if ((this.toolState?.tool ?? "select") !== "select") return;
      const p = this._coords(e);
      const hit = this.app.hitTest(p.x, p.y);
      if (hit == null) return;
      const layer = this.app.layers().find((l) => l.id === hit);
      let meta = null;
      try { meta = layer?.meta ? JSON.parse(layer.meta) : null; } catch { /* meta 없음 */ }
      if (meta?.type !== "text") {
        console.info("[text] 더블클릭한 레이어에 텍스트 meta 없음(이 빌드 이전에 만든 텍스트는 편집 불가):", layer?.name);
        return;
      }
      e.preventDefault();
      this.app.select(hit);
      // ★원본 트랜스폼 보존★ 커밋 시 offset/scale/rotation을 새 레이어로 이어받고,
      // 편집 박스도 변환된 실제 표시 위치에 띄운다(초기 생성 위치로 돌아가는 버그 수정).
      const xf = {
        offset: layer.offset ?? [0, 0],
        scale: layer.scale ?? [1, 1],
        rotation: layer.rotation ?? 0,
      };
      this._openText({
        x: meta.x, y: meta.y, value: meta.text,
        size: meta.size, rgba: meta.rgba,
        editId: hit, xf,
      });
    } catch (err) {
      console.error("[text] 편집 진입 실패:", err);
    }
  }
  updated() { this._applyZoom(); this._drawOverlay(); }

  // ---- 줌/팬 ----
  get zoom() { return this._zoom; }
  _applyZoom() {
    if (!this.base) return;
    const z = this._zoom;
    const vw = Math.max(1, Math.ceil(this.clientWidth / z));
    const vh = Math.max(1, Math.ceil(this.clientHeight / z));
    this.app.renderer.setViewport(this._origin.x, this._origin.y, vw, vh);
    this.base.style.width = `${vw * z}px`;
    this.base.style.height = `${vh * z}px`;
    this.base.style.imageRendering = z >= 1 ? "pixelated" : "auto";
    const ow = Math.max(1, this.clientWidth), oh = Math.max(1, this.clientHeight);
    if (this.overlay.width !== ow) this.overlay.width = ow;
    if (this.overlay.height !== oh) this.overlay.height = oh;
    this.overlay.style.width = `${ow}px`;
    this.overlay.style.height = `${oh}px`;
  }
  _setZoom(z, cx, cy) {
    const old = this._zoom;
    z = Math.min(8, Math.max(0.05, z));
    if (z === old) return;
    const rect = this.getBoundingClientRect();
    const px = (cx ?? rect.left + rect.width / 2) - rect.left;
    const py = (cy ?? rect.top + rect.height / 2) - rect.top;
    const world = { x: this._origin.x + px / old, y: this._origin.y + py / old };
    this._zoom = z;
    this._origin = { x: world.x - px / z, y: world.y - py / z };
    this._applyZoom();
    this._drawOverlay();
    this.dispatchEvent(new CustomEvent("zoom-changed", { detail: z, bubbles: true, composed: true }));
  }
  zoomCmd(action) {
    if (action === "in") this._setZoom(this._zoom * 1.25);
    else if (action === "out") this._setZoom(this._zoom / 1.25);
    else if (action === "reset") this._setZoom(1);
    else if (action === "fit") {
      const W = this.app.editor.width(), H = this.app.editor.height();
      const z = Math.min((this.clientWidth - 96) / W, (this.clientHeight - 96) / H);
      this._origin = { x: -48 / Math.max(z, 0.05), y: -48 / Math.max(z, 0.05) };
      this._setZoom(z);
    }
  }
  _wheel(e) {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      this._setZoom(this._zoom * Math.exp(-e.deltaY * 0.01), e.clientX, e.clientY);
      return;
    }
    e.preventDefault();
    const unit = e.deltaMode === WheelEvent.DOM_DELTA_LINE ? 16 : e.deltaMode === WheelEvent.DOM_DELTA_PAGE ? this.clientHeight : 1;
    const dx = (e.shiftKey && !e.deltaX ? e.deltaY : e.deltaX) * unit;
    const dy = (e.shiftKey && !e.deltaX ? 0 : e.deltaY) * unit;
    this._origin = {
      x: this._origin.x + dx / this._zoom,
      y: this._origin.y + dy / this._zoom,
    };
    this._applyZoom();
    this._drawOverlay();
  }

  // ---- 좌표 ----
  _coords(e) {
    const r = this.getBoundingClientRect();
    return {
      x: this._origin.x + (e.clientX - r.left) / this._zoom,
      y: this._origin.y + (e.clientY - r.top) / this._zoom,
    };
  }

  _screen(p) {
    return { x: (p.x - this._origin.x) * this._zoom, y: (p.y - this._origin.y) * this._zoom };
  }

  _frameAt(p) {
    const tol = 6 / (this._zoom || 1);
    for (const f of [...(this.app.frames?.() ?? [])].reverse()) {
      const inside = p.x >= f.x - tol && p.x <= f.x + f.w + tol && p.y >= f.y - tol && p.y <= f.y + f.h + tol;
      if (!inside) continue;
      const edge = Math.min(Math.abs(p.x - f.x), Math.abs(p.x - (f.x + f.w)), Math.abs(p.y - f.y), Math.abs(p.y - (f.y + f.h)));
      const label = p.x >= f.x && p.x <= f.x + 120 && p.y >= f.y - 24 && p.y <= f.y;
      if (edge <= tol || label) return f;
    }
    return null;
  }

  _context(e) {
    e.preventDefault();
    const p = this._coords(e);
    const host = this.getBoundingClientRect();
    const frame = this._frameAt(p);
    if (frame) {
      this.app.selectFrame(frame.id);
      this._ctx = { kind: "frame", x: e.clientX - host.left, y: e.clientY - host.top, id: frame.id };
      return;
    }
    const hit = this.app.hitTest(p.x, p.y);
    if (hit == null) {
      this._ctx = { kind: "canvas", x: e.clientX - host.left, y: e.clientY - host.top };
      return;
    }
    if (!this.app.selectedIds.includes(hit)) this.app.select(hit);
    this._ctx = { kind: "layer", x: e.clientX - host.left, y: e.clientY - host.top, id: hit };
  }

  _menuAction(fn) {
    this._ctx = null;
    fn();
  }

  // ---- 핸들 (화면 좌표 계산) ----
  /** 레이어 1개의 선택 지오메트리(드래그 중 임시값 미리보기 반영). */
  _selGeomFor(sel) {
    let b = this.app.layerBounds(sel.id);
    if (!b) return null;
    try {
      const meta = sel.meta ? JSON.parse(sel.meta) : null;
      const tb = meta?.type === "text" ? this.app.textBoxBounds(sel, meta) : null;
      if (tb) b = [tb.x, tb.y, tb.w, tb.h];
    } catch { /* non-text layer */ }
    const t = this.app.xformOf(sel);
    const d = this._drag;
    // 드래그 중 임시값 반영(미리보기). resize/rotate는 anchor 보정 offset도 함께.
    const prov = { ...sel };
    if (d?.mode === "move" && d.bases?.has(sel.id)) {
      const base = d.bases.get(sel.id);
      prov.offset = [base[0] + d.dx, base[1] + d.dy];
    }
    if (d?.mode === "resize" && d.id === sel.id) { prov.scale = d.provScale; if (d.provOffset) prov.offset = d.provOffset; }
    if (d?.mode === "rotate" && d.id === sel.id) { prov.rotation = d.provRot; if (d.provOffset) prov.offset = d.provOffset; }
    const tp = this.app.xformOf(prov);
    const z = this._zoom;
    const c4 = [
      tp.fwd(b[0], b[1]), tp.fwd(b[0] + b[2], b[1]),
      tp.fwd(b[0] + b[2], b[1] + b[3]), tp.fwd(b[0], b[1] + b[3]),
    ].map((p) => this._screen(p)); // 시계방향 TL,TR,BR,BL (화면px)
    const mid = (a, b2) => ({ x: (a.x + b2.x) / 2, y: (a.y + b2.y) / 2 });
    const handles = [
      { k: "tl", p: c4[0], ax: "xy" }, { k: "tm", p: mid(c4[0], c4[1]), ax: "y" },
      { k: "tr", p: c4[1], ax: "xy" }, { k: "mr", p: mid(c4[1], c4[2]), ax: "x" },
      { k: "br", p: c4[2], ax: "xy" }, { k: "bm", p: mid(c4[2], c4[3]), ax: "y" },
      { k: "bl", p: c4[3], ax: "xy" }, { k: "ml", p: mid(c4[3], c4[0]), ax: "x" },
    ];
    // 회전 핸들 스템은 **쿼드 자체 중심** 기준(상단 변에 수직). 변환 중심(문서 중심+offset)을
    // 쓰면 off-center 도형에서 스템이 기울어 보인다(버그 수정).
    const ctrS = {
      x: (c4[0].x + c4[1].x + c4[2].x + c4[3].x) / 4,
      y: (c4[0].y + c4[1].y + c4[2].y + c4[3].y) / 4,
    };
    const tmS = mid(c4[0], c4[1]);
    const dir = Math.hypot(tmS.x - ctrS.x, tmS.y - ctrS.y) || 1;
    const rot = { x: tmS.x + ((tmS.x - ctrS.x) / dir) * 20, y: tmS.y + ((tmS.y - ctrS.y) / dir) * 20 };
    return { sel, b, t, tp, c4, handles, rot, ctrS, z };
  }
  /** 핸들용 지오메트리 — 정확히 1개 선택일 때만(다중 선택은 핸들 비활성). */
  _selGeom() {
    if ((this.app.selectedIds?.length ?? 0) !== 1) return null;
    const sel = this.app.getSelected?.();
    if (!sel) return null;
    return this._selGeomFor(sel);
  }
  _hitHandle(e) {
    const g = this._selGeom();
    if (!g) return null;
    const r = this.getBoundingClientRect();
    const sx = e.clientX - r.left, sy = e.clientY - r.top;
    const near = (p, rad) => Math.hypot(p.x - sx, p.y - sy) <= rad;
    if (near(g.rot, HANDLE)) return { type: "rotate", g };
    for (const h of g.handles) if (near(h.p, HANDLE)) return { type: "resize", h, g };
    return null;
  }

  // ---- 스냅 가이드 ----
  /** 이동 스냅 타깃(doc px) — 비선택·표시 레이어의 AABB left/centerX/right(+세로 동등) + 문서 가장자리·중심.
   *  드래그 시작 시 한 번 수집(드래그 중 레이어 변형 없음). */
  _snapTargets(excludeIds) {
    const W = this.app.editor.width(), H = this.app.editor.height();
    const xs = [0, W / 2, W], ys = [0, H / 2, H];
    const skip = new Set(excludeIds ?? []);
    for (const l of this.app.layers()) {
      if (skip.has(l.id) || !l.visible) continue;
      const b = this.app.displayAABB(l);
      if (!b) continue;
      xs.push(b.x, b.x + b.w / 2, b.x + b.w);
      ys.push(b.y, b.y + b.h / 2, b.y + b.h);
    }
    return { xs, ys };
  }
  /** 한 축 스냅 — 이동 중 박스 모서리·중심(edges)과 타깃들 중 가장 가까운 쌍(임계 이내). */
  _snapAxis(edges, targets, thr) {
    let best = null;
    for (const ed of edges)
      for (const t of targets) {
        const diff = t - ed;
        if (Math.abs(diff) <= thr && (!best || Math.abs(diff) < Math.abs(best.diff))) best = { diff, t };
      }
    return best;
  }

  // ---- 포인터 ----
  _down(e) {
    if (this._ctx) this._ctx = null;
    if (this._space) {
      this._drag = { mode: "pan", sx: e.clientX, sy: e.clientY, ox: this._origin.x, oy: this._origin.y };
      this.style.cursor = "grabbing";
      return;
    }
    const p = this._coords(e);
    const tool = this.toolState?.tool ?? "select";

    if (tool === "eyedrop") {
      const W = this.app.editor.width();
      const buf = this.app.editor.composite_rgba();
      const i = (Math.floor(p.y) * W + Math.floor(p.x)) * 4;
      const rgba = [buf[i], buf[i + 1], buf[i + 2], 255];
      this.dispatchEvent(new CustomEvent("picked-color", { detail: rgba, bubbles: true, composed: true }));
      return;
    }
    if (tool === "brush") {
      this._drag = { mode: "brush", pts: [p.x, p.y], last: p };
      return;
    }
    if (tool === "frame") {
      this._drag = { mode: "frame", start: p, cur: p };
      return;
    }
    if (tool === "text") {
      // 캔버스가 포커스를 뺏어 textarea가 즉시 blur→사라지는 것 방지.
      e.preventDefault();
      // 방금 바깥 클릭으로 커밋이 일어났다면, 이 클릭은 '작업 종료'로 소비
      // (같은 클릭이 새 입력 박스를 또 열어 "박스가 이동"하는 것처럼 보이는 문제).
      if (performance.now() - (this._textDone ?? 0) < 350) return;
      this._openText({ x: p.x, y: p.y, value: "" });
      return;
    }
    if (tool === "select") {
      // 핸들 우선(선택 유지한 채 리사이즈/회전).
      const hh = this._hitHandle(e);
      if (hh?.type === "rotate") {
        const sel = hh.g.sel, b = hh.g.b;
        // 피벗 = 도형(불투명 bbox) 중심 — 회전해도 도형이 제자리에 머문다.
        const pivot = { x: b[0] + b[2] / 2, y: b[1] + b[3] / 2 };
        const pv = hh.g.t.fwd(pivot.x, pivot.y); // 피벗의 캔버스 위치(드래그 동안 고정 목표)
        const a0 = Math.atan2(p.y - pv.y, p.x - pv.x);
        this._drag = {
          mode: "rotate", id: sel.id, sel, pivot, pv, a0,
          rot0: sel.rotation ?? 0, provRot: sel.rotation ?? 0, provOffset: null,
        };
        return;
      }
      if (hh?.type === "resize") {
        const g = hh.g, sel = g.sel, b = g.b;
        // 핸들의 src 기준점(코너/엣지 중점).
        const hx = hh.h.k.includes("l") ? b[0] : hh.h.k.includes("r") ? b[0] + b[2] : b[0] + b[2] / 2;
        const hy = hh.h.k.startsWith("t") ? b[1] : hh.h.k.startsWith("b") ? b[1] + b[3] : b[1] + b[3] / 2;
        // anchor = 반대쪽 핸들(src) — Figma처럼 반대편이 고정된 채 늘어난다.
        const ax2 = hh.h.k.includes("l") ? b[0] + b[2] : hh.h.k.includes("r") ? b[0] : b[0] + b[2] / 2;
        const ay2 = hh.h.k.startsWith("t") ? b[1] + b[3] : hh.h.k.startsWith("b") ? b[1] : b[1] + b[3] / 2;
        // ★스케일 유도(anchor 기준 — 분모가 bbox 크기라 0 폭발 없음)★
        // off'가 anchor를 고정하므로 T'(h)−T'(a) = R(S'⊙(h−a)) = p − A
        // → S' = R⁻¹(p − A) ⊘ (h − a),  A = 드래그 시작 시 anchor의 캔버스 위치.
        this._drag = {
          mode: "resize", id: sel.id, ax: hh.h.ax,
          scale0: sel.scale ?? [1, 1], provScale: sel.scale ?? [1, 1], provOffset: null,
          hsrc: { x: hx, y: hy }, asrc: { x: ax2, y: ay2 },
          A: g.t.fwd(ax2, ay2), sel,
        };
        return;
      }
      // 더블클릭 추정(텍스트 편집)은 dblclick 핸들러에서 처리.
      const hit = this.app.hitTest(p.x, p.y);
      // Shift+클릭 = 선택 토글(드래그 없이).
      if (e.shiftKey) {
        if (hit != null) this.app.toggleSelect(hit);
        return;
      }
      if (hit == null) {
        const frame = this._frameAt(p);
        if (frame) {
          this.app.selectFrame(frame.id);
          return;
        }
        // 빈 공간 드래그 = 마퀴 다중선택(클릭이면 해제 — _end에서 판정).
        this._drag = { mode: "marquee", start: p, cur: p };
        return;
      }
      // 이미 선택된 레이어를 잡으면 선택 유지(다중 함께 이동), 아니면 단일 교체.
      if (!this.app.selectedIds.includes(hit)) this.app.select(hit);
      const all = this.app.layers();
      const bases = new Map();
      let box = null; // 선택 레이어들의 합쳐진 AABB(스냅 기준 — 이동 중엔 dx/dy만 더하면 됨).
      for (const id of this.app.selectedIds) {
        const l = all.find((v) => v.id === id);
        if (!l) continue;
        bases.set(id, l.offset ?? [0, 0]);
        const bb = this.app.displayAABB(l);
        if (!bb) continue;
        if (!box) box = { ...bb };
        else {
          const x2 = Math.max(box.x + box.w, bb.x + bb.w), y2 = Math.max(box.y + box.h, bb.y + bb.h);
          box.x = Math.min(box.x, bb.x); box.y = Math.min(box.y, bb.y);
          box.w = x2 - box.x; box.h = y2 - box.y;
        }
      }
      this._drag = {
        mode: "move", ids: [...this.app.selectedIds], bases, start: p, dx: 0, dy: 0,
        box, snap: this._snapTargets(this.app.selectedIds), guides: null,
      };
      this.style.cursor = "grabbing";
      return;
    }
    // 그리기 도구
    this._drag = { mode: "draw", start: p, cur: p };
  }
  _move(e) {
    const d = this._drag;
    if (!d) { this._hover(e); return; }
    if (d.mode === "pan") {
      this._origin = {
        x: d.ox - (e.clientX - d.sx) / this._zoom,
        y: d.oy - (e.clientY - d.sy) / this._zoom,
      };
      this._applyZoom();
      this._drawOverlay();
      return;
    }
    const p = this._coords(e);
    if (d.mode === "move") {
      d.dx = Math.round(p.x - d.start.x);
      d.dy = Math.round(p.y - d.start.y);
      // ★스냅★ 합쳐진 AABB의 left/centerX/right(+세로)를 타깃과 비교, 화면 6px 이내면 보정.
      d.guides = null;
      if (d.box && d.snap) {
        const thr = 6 / (this._zoom || 1);
        const bx = this._snapAxis(
          [d.box.x + d.dx, d.box.x + d.box.w / 2 + d.dx, d.box.x + d.box.w + d.dx], d.snap.xs, thr);
        const by = this._snapAxis(
          [d.box.y + d.dy, d.box.y + d.box.h / 2 + d.dy, d.box.y + d.box.h + d.dy], d.snap.ys, thr);
        if (bx) d.dx = Math.round(d.dx + bx.diff);
        if (by) d.dy = Math.round(d.dy + by.diff);
        if (bx || by) d.guides = { x: bx ? [bx.t] : [], y: by ? [by.t] : [] };
      }
      this._drawOverlay();
    } else if (d.mode === "marquee") {
      d.cur = p;
      this._drawOverlay();
    } else if (d.mode === "rotate") {
      const a = Math.atan2(p.y - d.pv.y, p.x - d.pv.x);
      let deg = d.rot0 + ((a - d.a0) * 180) / Math.PI;
      if (e.shiftKey) deg = Math.round(deg / 15) * 15; // Shift = 15° 스냅
      d.provRot = Math.round(deg * 10) / 10;
      // 도형 중심(피벗)이 제자리에 머물도록 offset 보정.
      d.provOffset = this.app.computeAnchoredOffset(d.sel, null, d.provRot, d.pivot);
      this._drawOverlay();
    } else if (d.mode === "resize") {
      const sel = d.sel;
      // anchor 기준 유도: S' = R⁻¹(p − A) ⊘ (h − a). 분모 = bbox 폭/높이(0 폭발 없음).
      const rad = ((sel.rotation ?? 0) * Math.PI) / 180;
      const cos = Math.cos(rad), sin = Math.sin(rad);
      const vx = p.x - d.A.x, vy = p.y - d.A.y;
      const lx = cos * vx + sin * vy; // R⁻¹
      const ly = -sin * vx + cos * vy;
      let [nsx, nsy] = d.scale0;
      const bx = d.hsrc.x - d.asrc.x, by = d.hsrc.y - d.asrc.y;
      if (d.ax.includes("x") && Math.abs(bx) > 1e-3) nsx = lx / bx;
      if (d.ax.includes("y") && Math.abs(by) > 1e-3) nsy = ly / by;
      if (e.shiftKey && d.ax === "xy") {
        // Shift = 비율 고정(원본 비율 기준 큰 쪽).
        const r0 = d.scale0[1] / (d.scale0[0] || 1);
        if (Math.abs(nsx * r0) > Math.abs(nsy)) nsy = nsx * r0; else nsx = nsy / r0;
      }
      const clampS = (v) => (Math.abs(v) < 0.01 ? (v < 0 ? -0.01 : 0.01) : v);
      d.provScale = [clampS(Math.round(nsx * 1000) / 1000), clampS(Math.round(nsy * 1000) / 1000)];
      // 반대쪽 핸들(anchor)이 제자리에 머물도록 offset 보정.
      d.provOffset = this.app.computeAnchoredOffset(d.sel, d.provScale, null, d.asrc);
      this._drawOverlay();
    } else if (d.mode === "brush") {
      // 점 간 최소 간격(줌 보정)으로 thinning — 과밀 점 방지.
      const minD = 1.5 / (this._zoom || 1);
      if (Math.hypot(p.x - d.last.x, p.y - d.last.y) >= minD) {
        d.pts.push(p.x, p.y);
        d.last = p;
        this._drawBrushGhost();
      }
    } else if (d.mode === "frame") {
      d.cur = p;
      this._drawOverlay();
    } else if (d.mode === "draw") {
      d.cur = p;
      this._drawGhost();
    }
  }

  _hover(e) {
    if ((this.toolState?.tool ?? "select") !== "select" || this._space || this._text) {
      if (this._hoverId != null) { this._hoverId = null; this._drawOverlay(); }
      return;
    }
    const p = this._coords(e);
    const hit = this.app.hitTest(p.x, p.y);
    const next = hit == null || this.app.selectedIds?.includes(hit) ? null : hit;
    if (next !== this._hoverId) {
      this._hoverId = next;
      this._drawOverlay();
    }
  }
  _end() {
    const d = this._drag;
    if (!d) return;
    this._drag = null;
    this.style.cursor = this._space ? "grab" : "";
    if (d.mode === "pan") return;
    if (d.mode === "brush") {
      const s = this.toolState;
      if (d.pts.length >= 2) {
        this.app.apply([
          B.addPaintLayer("brush", B.shapes([B.path(d.pts, s.width, s.rgba)]), { bind: "drawn" }),
          B.setProps("drawn", { meta: JSON.stringify({ type: "brush" }) }),
        ]);
      }
      this._drawOverlay();
      return;
    }
    if (d.mode === "frame") {
      const x = Math.min(d.start.x, d.cur.x), y = Math.min(d.start.y, d.cur.y);
      const w = Math.abs(d.cur.x - d.start.x), h = Math.abs(d.cur.y - d.start.y);
      if (w >= 8 && h >= 8) {
        const n = this.app.frames().length + 1;
        this.app.addFrame(`Frame ${n}`, x, y, w, h);
      }
      this._drawOverlay();
      return;
    }
    if (d.mode === "move") {
      if (d.dx !== 0 || d.dy !== 0) {
        // 선택된 모든 레이어를 하나의 apply 배치로 함께 이동.
        const acts = d.ids
          .map((id) => {
            const base = d.bases.get(id);
            return base ? B.setOffset(id, [base[0] + d.dx, base[1] + d.dy]) : null;
          })
          .filter(Boolean);
        if (acts.length) this.app.apply(acts);
        else this._drawOverlay();
      } else this._drawOverlay();
      return;
    }
    if (d.mode === "marquee") {
      const x0 = Math.min(d.start.x, d.cur.x), y0 = Math.min(d.start.y, d.cur.y);
      const mw = Math.abs(d.cur.x - d.start.x), mh = Math.abs(d.cur.y - d.start.y);
      if (mw < 2 && mh < 2) {
        // 드래그 없는 빈 공간 클릭 = 선택 해제(기존 동작 유지).
        this.app.select(null);
        return;
      }
      // displayAABB가 마퀴 사각형과 교차하는 모든 레이어 선택.
      const ids = [];
      for (const l of this.app.layers()) {
        const box = this.app.displayAABB(l);
        if (box && box.x < x0 + mw && box.x + box.w > x0 && box.y < y0 + mh && box.y + box.h > y0)
          ids.push(l.id);
      }
      this.app.selectMany(ids);
      return;
    }
    if (d.mode === "rotate") {
      if (d.provRot !== d.rot0)
        this.app.apply([B.setProps(d.id, { rotation: d.provRot, offset: d.provOffset ?? undefined })]);
      else this._drawOverlay();
      return;
    }
    if (d.mode === "resize") {
      if (d.provScale[0] !== d.scale0[0] || d.provScale[1] !== d.scale0[1])
        this.app.apply([B.setProps(d.id, { scale: d.provScale, offset: d.provOffset ?? undefined })]);
      else this._drawOverlay();
      return;
    }
    // draw 확정
    const o = this.overlay.getContext("2d");
    o.clearRect(0, 0, this.overlay.width, this.overlay.height);
    const { start, cur } = d;
    const s = this.toolState; const rgba = s.rgba;
    const bx = Math.min(start.x, cur.x), by = Math.min(start.y, cur.y);
    const bw = Math.abs(cur.x - start.x), bh = Math.abs(cur.y - start.y);
    const ecx = (start.x + cur.x) / 2, ecy = (start.y + cur.y) / 2;
    let shape, name;
    switch (s.tool) {
      case "rect": if (bw < 1 || bh < 1) return; shape = B.rect(bx, by, bw, bh, rgba); name = "rect"; break;
      case "ellipse": if (bw < 1 || bh < 1) return; shape = B.ellipse(ecx, ecy, bw / 2, bh / 2, rgba); name = "ellipse"; break;
      case "stroke-rect": if (bw < 1 || bh < 1) return; shape = B.strokeRect(bx, by, bw, bh, s.width, rgba); name = "stroke-rect"; break;
      case "stroke-ellipse": if (bw < 1 || bh < 1) return; shape = B.strokeEllipse(ecx, ecy, bw / 2, bh / 2, s.width, rgba); name = "stroke-ellipse"; break;
      case "rounded-rect": if (bw < 1 || bh < 1) return; shape = B.roundedRect(bx, by, bw, bh, s.radius, rgba); name = "rounded-rect"; break;
      case "line":
        if (Math.hypot(cur.x - start.x, cur.y - start.y) < 1) return;
        shape = B.line(start.x, start.y, cur.x, cur.y, s.width, rgba); name = "line"; break;
      default: return;
    }
    this.app.apply([
      B.addPaintLayer(name, B.shapes([shape]), { bind: "drawn" }),
      B.setProps("drawn", { meta: JSON.stringify({ type: "shape", shape: s.tool }) }),
    ]);
  }

  // ---- 텍스트 입력 커밋 ----
  _commitText() {
    const t = this._text;
    if (!t) return;
    // 생성 직후 의도치 않은 blur(포커스 경합)는 무시 — 입력 기회 보존(박스 유지).
    if (!t.value && performance.now() - (t.born ?? 0) < 250) {
      this.updateComplete.then(() => this.renderRoot.querySelector("textarea.txt")?.focus());
      return;
    }
    this._text = null;
    const v = t.value.replace(/\s+$/, "");
    if (!v) { this._finishText(); return; }
    const s = this.toolState ?? {};
    const size = t.size ?? s.size ?? 32;
    const rgba = t.rgba ?? s.rgba ?? [13, 153, 255, 255];
    const meta = JSON.stringify({ type: "text", x: t.x, y: t.y, text: v, size, rgba });
    const name = v.split("\n")[0].slice(0, 20);
    if (t.editId != null) {
      // 기존 텍스트 편집: 같은 z-순서에 재래스터(삭제 후 그 인덱스로 추가) + meta 갱신.
      // ★원본 트랜스폼(offset/scale/rotation) 이어받기 — 이동해 둔 위치 보존★
      const idx = this.app.orderBottomToTop().indexOf(t.editId);
      const xf = t.xf ?? { offset: [0, 0], scale: [1, 1], rotation: 0 };
      this.app.apply([
        B.deleteLayer(t.editId),
        B.addPaintLayer(name, B.shapes([B.text(t.x, t.y, v, size, rgba)]), { index: idx >= 0 ? idx : undefined, bind: "t" }),
        B.setProps("t", { meta, offset: xf.offset, scale: xf.scale, rotation: xf.rotation }),
      ]);
      this.app.select(null);
    } else {
      this.app.apply([
        B.addPaintLayer(name, B.shapes([B.text(t.x, t.y, v, size, rgba)]), { bind: "t" }),
        B.setProps("t", { meta }),
      ]);
    }
    this._finishText();
  }

  // ---- 오버레이 ----
  _drawGhost() {
    const o = this.overlay.getContext("2d");
    const z = this._zoom;
    o.clearRect(0, 0, this.overlay.width, this.overlay.height);
    this._drawFrames(o, z);
    const s = this.toolState; const [r, g, b, a] = s.rgba;
    o.strokeStyle = `rgba(${r},${g},${b},${a / 255})`;
    o.fillStyle = `rgba(${r},${g},${b},${(a / 255) * 0.45})`;
    o.lineWidth = 1; o.setLineDash([4, 3]);
    const { start, cur } = this._drag;
    const s0 = this._screen(start), s1 = this._screen(cur);
    const bx = Math.min(s0.x, s1.x), by = Math.min(s0.y, s1.y);
    const bw = Math.abs(cur.x - start.x) * z, bh = Math.abs(cur.y - start.y) * z;
    switch (s.tool) {
      case "rect": o.fillRect(bx, by, bw, bh); o.strokeRect(bx, by, bw, bh); break;
      case "ellipse": o.beginPath(); o.ellipse(bx + bw / 2, by + bh / 2, bw / 2, bh / 2, 0, 0, 7); o.fill(); o.stroke(); break;
      case "stroke-rect": o.setLineDash([]); o.lineWidth = s.width * z; o.strokeRect(bx, by, bw, bh); break;
      case "stroke-ellipse": o.setLineDash([]); o.lineWidth = s.width * z; o.beginPath(); o.ellipse(bx + bw / 2, by + bh / 2, bw / 2, bh / 2, 0, 0, 7); o.stroke(); break;
      case "rounded-rect": o.beginPath(); if (o.roundRect) o.roundRect(bx, by, bw, bh, s.radius * z); else o.rect(bx, by, bw, bh); o.fill(); o.stroke(); break;
      default: o.setLineDash([]); o.lineWidth = s.width * z; o.beginPath(); o.moveTo(s0.x, s0.y); o.lineTo(s1.x, s1.y); o.stroke();
    }
    o.setLineDash([]);
  }
  /** 브러시 진행 중 폴리라인 ghost. */
  _drawBrushGhost() {
    const o = this.overlay.getContext("2d");
    const z = this._zoom;
    o.clearRect(0, 0, this.overlay.width, this.overlay.height);
    this._drawFrames(o, z);
    const d = this._drag, s = this.toolState;
    if (!d || d.pts.length < 2) return;
    const [r, g, b, a] = s.rgba;
    o.strokeStyle = `rgba(${r},${g},${b},${a / 255})`;
    o.lineWidth = s.width * z; o.lineCap = "round"; o.lineJoin = "round";
    o.beginPath();
    let p0 = this._screen({ x: d.pts[0], y: d.pts[1] });
    o.moveTo(p0.x, p0.y);
    for (let i = 2; i < d.pts.length; i += 2) {
      const p = this._screen({ x: d.pts[i], y: d.pts[i + 1] });
      o.lineTo(p.x, p.y);
    }
    o.stroke();
  }

  /** Frame 외곽선 + 이름 라벨(항상 표시 — Figma의 캔버스). */
  _drawFrames(o, z) {
    const frames = this.app.frames?.() ?? [];
    const fg = getComputedStyle(this).getPropertyValue("--fg-3").trim() || "#7f7f7f";
    o.save();
    o.font = "11px Inter, sans-serif";
    for (const f of frames) {
      o.strokeStyle = fg; o.lineWidth = 1; o.setLineDash([]);
      const p = this._screen({ x: f.x, y: f.y });
      if (this.app.selectedFrameId === f.id) {
        o.fillStyle = "rgba(135,185,207,0.08)";
        o.fillRect(p.x, p.y, f.w * z, f.h * z);
        o.strokeStyle = getComputedStyle(this).getPropertyValue("--accent").trim() || "#87b9cf";
        o.lineWidth = 1.5;
      }
      o.strokeRect(p.x + 0.5, p.y + 0.5, f.w * z, f.h * z);
      o.fillStyle = fg;
      o.fillText(f.name, p.x, p.y - 5);
    }
    // 프레임 드래그 중 미리보기.
    const d = this._drag;
    if (d?.mode === "frame") {
      const s0 = this._screen(d.start), s1 = this._screen(d.cur);
      const x = Math.min(s0.x, s1.x), y = Math.min(s0.y, s1.y);
      const w = Math.abs(d.cur.x - d.start.x) * z, h = Math.abs(d.cur.y - d.start.y) * z;
      o.setLineDash([5, 4]);
      o.strokeStyle = fg;
      o.strokeRect(x, y, w, h);
      o.setLineDash([]);
    }
    o.restore();
  }

  _drawOverlay() {
    if (!this.overlay) return;
    const o = this.overlay.getContext("2d");
    o.clearRect(0, 0, this.overlay.width, this.overlay.height);
    this._drawFrames(o, this._zoom);
    if (this._text) return; // 텍스트 편집 중엔 셀렉션 크롬 숨김(입력에 집중).
    if (this._drag?.mode === "draw") { this._drawGhost(); return; }
    if (this._drag?.mode === "brush") { this._drawBrushGhost(); return; }
    const cs = getComputedStyle(this);
    const acc = cs.getPropertyValue("--accent-strong").trim() || cs.getPropertyValue("--accent").trim() || "#0b87e0";
    // 마퀴(점선 사각형, --accent 색).
    if (this._drag?.mode === "marquee") {
      const d = this._drag, z = this._zoom;
      const s0 = this._screen(d.start), s1 = this._screen(d.cur);
      const x = Math.min(s0.x, s1.x), y = Math.min(s0.y, s1.y);
      const w = Math.abs(d.cur.x - d.start.x) * z, h = Math.abs(d.cur.y - d.start.y) * z;
      o.setLineDash([4, 3]);
      o.strokeStyle = acc; o.lineWidth = 1;
      o.fillStyle = "rgba(13,153,255,0.06)";
      o.fillRect(x, y, w, h);
      o.strokeRect(x, y, w, h);
      o.setLineDash([]);
    }
    // 선택된 각 레이어에 셀렉션 쿼드(회전 반영). 핸들은 단일 선택일 때만.
    const single = (this.app.selectedIds?.length ?? 0) === 1;
    if (this._hoverId != null) {
      const hov = this.app.layers().find((l) => l.id === this._hoverId);
      const hg = hov ? this._selGeomFor(hov) : null;
      if (hg) {
        o.beginPath();
        o.moveTo(hg.c4[0].x, hg.c4[0].y);
        for (let i = 1; i < 4; i++) o.lineTo(hg.c4[i].x, hg.c4[i].y);
        o.closePath();
        o.strokeStyle = acc;
        o.lineWidth = 1.5;
        o.stroke();
      }
    }
    for (const sel of this.app.selectedLayers?.() ?? []) {
      const g = this._selGeomFor(sel);
      if (!g) continue;
      o.beginPath();
      o.moveTo(g.c4[0].x, g.c4[0].y);
      for (let i = 1; i < 4; i++) o.lineTo(g.c4[i].x, g.c4[i].y);
      o.closePath();
      o.fillStyle = "rgba(13,153,255,0.08)";
      o.fill();
      o.strokeStyle = acc; o.lineWidth = 2;
      o.stroke();
      if (!single) continue;
      // 회전 핸들(상단 중앙 바깥 원).
      const tm = { x: (g.c4[0].x + g.c4[1].x) / 2, y: (g.c4[0].y + g.c4[1].y) / 2 };
      o.beginPath(); o.moveTo(tm.x, tm.y); o.lineTo(g.rot.x, g.rot.y); o.stroke();
      o.beginPath(); o.arc(g.rot.x, g.rot.y, 4.5, 0, 7); o.fillStyle = "#fff"; o.fill(); o.stroke();
      // 리사이즈 핸들 8개(흰 채움 + 액센트 보더).
      for (const h of g.handles) {
        o.fillStyle = "#fff";
        o.fillRect(h.p.x - HANDLE / 2, h.p.y - HANDLE / 2, HANDLE, HANDLE);
        o.strokeStyle = acc; o.lineWidth = 1;
        o.strokeRect(h.p.x - HANDLE / 2 + 0.5, h.p.y - HANDLE / 2 + 0.5, HANDLE - 1, HANDLE - 1);
      }
    }
    // 스냅 가이드(move 드래그가 스냅 중일 때만) — 1px 빨간 라인, 캔버스 전체 길이.
    const gd = this._drag?.mode === "move" ? this._drag.guides : null;
    if (gd) {
      const z = this._zoom;
      o.strokeStyle = "#f24822"; o.lineWidth = 1;
      for (const gx of gd.x) {
        const px = Math.round((gx - this._origin.x) * z) + 0.5;
        o.beginPath(); o.moveTo(px, 0); o.lineTo(px, this.overlay.height); o.stroke();
      }
      for (const gy of gd.y) {
        const py = Math.round((gy - this._origin.y) * z) + 0.5;
        o.beginPath(); o.moveTo(0, py); o.lineTo(this.overlay.width, py); o.stroke();
      }
    }
  }
  render() {
    const z = this._zoom;
    const t = this._text;
    const s = this.toolState;
    return html`
      <div class="wrap">
        <canvas id="base"></canvas><canvas id="overlay"></canvas>
        ${this._ctx ? html`
          <div class="ctx" style="left:${this._ctx.x}px; top:${this._ctx.y}px">
            ${this._ctx.kind === "canvas" ? html`
              <button class=${this._bgMode === "dot" ? "active" : ""} @click=${() => this._menuAction(() => this._setBgMode("dot"))}>${icon("plus", 13)}도트 격자</button>
              <button class=${this._bgMode === "solid" ? "active" : ""} @click=${() => this._menuAction(() => this._setBgMode("solid"))}>${icon("squareFill", 13)}단색 그레이</button>
            ` : this._ctx.kind === "frame" ? (() => {
              const f = this.app.frames().find((v) => v.id === this._ctx.id);
              const id = this._ctx.id;
              return html`
                <button @click=${() => this._menuAction(() => f && this.app.exportFrame(f))}>${icon("download", 13)}프레임 PNG</button>
                <button @click=${() => this._menuAction(() => this.app.removeFrame(id))}>${icon("trash", 13)}프레임 삭제</button>`;
            })() : html`
              <button @click=${() => this._menuAction(() => this.app.duplicateMany(this.app.selectedIds))}>${icon("dup", 13)}복제</button>
              <button ?disabled=${this.app.selectedIds.length < 2} @click=${() => this._menuAction(() => this.app.groupSelected())}>${icon("folder", 13)}그룹</button>
              <button ?disabled=${this.app.getSelected()?.kind !== "group"} @click=${() => this._menuAction(() => this.app.ungroupSelected())}>${icon("dup", 13)}그룹 해제</button>
              <div class="hr"></div>
              <button @click=${() => this._menuAction(() => this.app.raiseMany(this.app.selectedIds))}>${icon("chevUpS", 13)}앞으로</button>
              <button @click=${() => this._menuAction(() => this.app.lowerMany(this.app.selectedIds))}>${icon("chevDownS", 13)}뒤로</button>
              <div class="hr"></div>
              <button @click=${() => this._menuAction(() => this.app.deleteMany(this.app.selectedIds))}>${icon("trash", 13)}삭제</button>
            `}
          </div>` : nothing}
        ${t ? (() => {
          // 편집 박스는 레이어의 **변환된 표시 위치**에(원시 meta 좌표 아님 — 위치 버그 수정).
          const xf = t.xf;
          const pos = xf
            ? this.app.xformOf({ offset: xf.offset, scale: xf.scale, rotation: xf.rotation }).fwd(t.x, t.y)
            : { x: t.x, y: t.y };
          const sp = this._screen(pos);
          const [sxx, syy] = xf?.scale ?? [1, 1];
          const rotDeg = xf?.rotation ?? 0;
          return html`<textarea class="txt" spellcheck="false"
          style="left:${sp.x}px; top:${sp.y}px; font-size:${(t.size ?? s?.size ?? 32) * z}px; color:${HEX(t.rgba ?? s?.rgba ?? [13, 153, 255, 255])}; transform: rotate(${rotDeg}deg) scale(${sxx}, ${syy}); transform-origin: 0 0;"
          .value=${t.value}
          @input=${(e) => { t.value = e.target.value; e.target.style.width = "auto"; e.target.style.width = e.target.scrollWidth + "px"; e.target.style.height = "auto"; e.target.style.height = e.target.scrollHeight + "px"; }}
          @keydown=${(e) => {
            if (e.key === "Escape") { this._text = null; this._finishText(); }
            else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) this._commitText();
            e.stopPropagation();
          }}
          @blur=${() => this._commitText()}></textarea>`;
        })() : nothing}
      </div>
    `;
  }
}
customElements.define("dx-canvas", DxCanvas);

// ───────── 레이어 패널 (좌측) ─────────
class DxLayerPanel extends LitElement {
  static properties = {
    app: { attribute: false }, _v: { state: true }, _menu: { state: true },
    _editing: { state: true }, _dragId: { state: true }, _dropId: { state: true },
    _ctx: { state: true }, _projectMenu: { state: true }, _query: { state: true },
  };
  static styles = [controls, css`
    :host {
      grid-area: layers; display: flex; flex-direction: column; width: 240px;
      background: var(--bg-panel); border-right: 1px solid var(--line); overflow: hidden;
      z-index: 25;
    }
    .head {
      padding: 12px 12px 8px; font-size: 11px; font-weight: 600; color: var(--fg);
      display: flex; align-items: center; justify-content: space-between;
      gap: 8px;
    }
    .head .add { width: 24px; height: 24px; padding: 0; justify-content: center; }
    .head-title { flex: none; }
    .head-actions { display: flex; gap: 2px; }
    .search {
      flex: 1; min-width: 0; height: 24px; padding: 0 7px;
      font-size: 11px; background: var(--bg-elev); border: 1px solid var(--line-soft);
      border-radius: 6px; color: var(--fg);
    }
    .search::placeholder { color: var(--fg-3); }
    .project {
      position: relative; display: flex; align-items: center; gap: 8px;
      min-height: 48px; padding: 0 10px 0 12px; border-bottom: 1px solid var(--line);
    }
    .project-name {
      flex: 1; min-width: 0; color: var(--fg); font-size: 12px; font-weight: 600;
      overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    }
    .project .add { width: 28px; height: 28px; padding: 0; justify-content: center; flex: none; }
    .list { flex: 1; overflow-y: auto; padding: 0 6px 8px; position: relative; }
    .menu, .ctx, .project-menu {
      position: absolute; right: 10px; top: 2px; z-index: 40; background: var(--bg-panel);
      border: 1px solid var(--line); border-radius: 9px; padding: 5px; min-width: 160px;
      box-shadow: var(--shadow-menu); display: flex; flex-direction: column; gap: 1px;
    }
    .project-menu { top: 38px; min-width: 180px; }
    .ctx { right: auto; }
    .menu button, .ctx button, .project-menu button { width: 100%; justify-content: flex-start; height: 30px; color: var(--fg); }
    .menu button:hover, .ctx button:hover, .project-menu button:hover { background: var(--accent); color: #fff; }
    .ctx .hr, .project-menu .hr { height: 1px; background: var(--line-soft); margin: 4px 3px; }
    .row {
      display: flex; align-items: center; gap: 7px; padding: 0 6px; height: 32px;
      border-radius: var(--radius); font-size: 11.5px; cursor: default; color: var(--fg-2);
    }
    .row:hover { background: var(--bg-hover); color: var(--fg); }
    .row.sel { background: var(--accent-soft); color: var(--fg); }
    .row.dragging { opacity: 0.45; }
    .row.drop { box-shadow: inset 0 2px 0 var(--accent-strong); }
    .row .tic { color: var(--fg-3); flex: none; display: flex; }
    .row.sel .tic { color: var(--accent); }
    .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .name input { height: 22px; width: 100%; font-size: 11.5px; padding: 0 5px; }
    .ord { display: flex; flex-direction: column; flex: none; opacity: 0; }
    .ord button { height: 11px; padding: 0 2px; border-radius: 3px; }
    .b { opacity: 0; width: 24px; height: 24px; padding: 0; justify-content: center; flex: none; }
    .row:hover .b, .row.sel .b, .row:hover .ord { opacity: 1; }
    .b.danger:hover { background: var(--danger); color: #fff; }
    .empty { padding: 18px 12px; color: var(--fg-3); font-size: 11px; line-height: 1.7; }
  `];
  constructor() {
    super();
    this._v = 0; this._menu = false; this._editing = null;
    this._dragId = null; this._dropId = null; this._ctx = null; this._projectMenu = false; this._query = "";
  }
  connectedCallback() {
    super.connectedCallback();
    this._onChange = () => { this._v++; };
    this.app?.addEventListener("changed", this._onChange);
    this._onDoc = (e) => {
      if ((this._menu || this._ctx || this._projectMenu) && !e.composedPath().includes(this)) {
        this._menu = false;
        this._ctx = null;
        this._projectMenu = false;
      }
    };
    document.addEventListener("pointerdown", this._onDoc);
  }
  disconnectedCallback() {
    this.app?.removeEventListener("changed", this._onChange);
    document.removeEventListener("pointerdown", this._onDoc);
    super.disconnectedCallback();
  }
  _addLayer(color) {
    this._menu = false;
    this.app.apply([B.addPaintLayer(color ? "fill" : "layer", color ? B.fill(color) : B.transparent())]);
  }
  _addPng(e) {
    this._menu = false;
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const b64 = String(reader.result).split(",")[1] || "";
      this.app.apply([
        B.addPaintLayer(file.name.replace(/\.[^.]+$/, ""), B.pngBase64(b64), { bind: "img" }),
        B.setProps("img", { meta: JSON.stringify({ type: "image" }) }),
      ]);
    };
    reader.readAsDataURL(file);
    e.target.value = "";
  }
  _rename(id, v) {
    this._editing = null;
    const name = v.trim();
    if (name) this.app.apply([B.setProps(id, { name })]);
  }
  _dropOn(targetId) {
    const dragId = this._dragId;
    this._dragId = null;
    this._dropId = null;
    if (dragId == null || targetId == null || dragId === targetId) return;
    const order = this.app.orderBottomToTop();
    const from = order.indexOf(dragId);
    const to = order.indexOf(targetId);
    if (from < 0 || to < 0) return;
    this.app.apply([B.moveLayer(dragId, to)]);
  }
  _openContext(e, l, depth) {
    e.preventDefault();
    e.stopPropagation();
    const list = this.renderRoot.querySelector(".list")?.getBoundingClientRect();
    const base = list ?? this.getBoundingClientRect();
    const x = Math.min(Math.max(4, e.clientX - base.left), Math.max(8, base.width - 176));
    const y = Math.min(Math.max(4, e.clientY - base.top), Math.max(8, base.height - 208));
    if (l.itemType === "frame") {
      this.app.selectFrame(l.id);
      this._menu = false;
      this._ctx = {
        kind: "frame",
        id: l.id,
        depth,
        x,
        y,
      };
      return;
    }
    if (!this.app.selectedIds.includes(l.id)) this.app.select(l.id);
    this._menu = false;
    this._ctx = {
      kind: "layer",
      id: l.id,
      depth,
      x,
      y,
    };
  }
  _menuAction(fn) {
    this._ctx = null;
    fn();
  }
  _projectAction(fn) {
    this._projectMenu = false;
    fn();
  }
  async _renameProject(currentName) {
    const next = prompt("새 프로젝트 이름", currentName)?.trim();
    if (!next || next === currentName) return;
    try {
      const r = await fetch(`/projects/${encodeURIComponent(currentName)}/rename`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name: next }),
      });
      if (!r.ok) throw new Error(await r.text());
      location.search = `?doc=${encodeURIComponent(next)}`;
    } catch (e) {
      alert(`프로젝트 이름 변경 실패: ${e.message}`);
    }
  }
  _frameContains(frame, layer) {
    const b = this.app.displayAABB(layer);
    if (!b) return false;
    const cx = b.x + b.w / 2;
    const cy = b.y + b.h / 2;
    return cx >= frame.x && cx <= frame.x + frame.w && cy >= frame.y && cy <= frame.y + frame.h;
  }
  _items() {
    const layers = this.app ? this.app.layerTree() : [];
    const frames = this.app?.frames?.() ?? [];
    const frameNodes = frames.map((f) => ({ ...f, itemType: "frame", children: [] }));
    const claimed = new Set();
    for (const l of layers) {
      const f = [...frameNodes].reverse().find((v) => this._frameContains(v, l));
      if (!f) continue;
      f.children.push(l);
      claimed.add(l.id);
    }
    return [
      ...frameNodes,
      ...layers.filter((l) => !claimed.has(l.id)),
    ];
  }
  _filterItems(items, q) {
    const needle = q.trim().toLowerCase();
    if (!needle) return items;
    const visit = (item) => {
      const children = (item.children ?? []).map(visit).filter(Boolean);
      const matched = String(item.name ?? "").toLowerCase().includes(needle);
      return matched || children.length ? { ...item, children } : null;
    };
    return items.map(visit).filter(Boolean);
  }
  _layerIcon(l) {
    if (l.itemType === "frame") return "frame";
    if (l.kind === "group") return "folder";
    let meta = null;
    try { meta = l.meta ? JSON.parse(l.meta) : null; } catch { /* ignore */ }
    if (meta?.type === "text") return "text";
    if (meta?.type === "image") return "image";
    if (meta?.type === "brush") return "pencil";
    const shape = meta?.shape || String(l.name ?? "").toLowerCase();
    if (shape.includes("ellipse")) return shape.includes("stroke") ? "circle" : "circleFill";
    if (shape.includes("line")) return "line";
    if (shape.includes("rounded")) return "rounded";
    if (shape.includes("rect")) return shape.includes("stroke") ? "square" : "squareFill";
    if (shape.includes("brush")) return "pencil";
    if (shape.includes("fill")) return "squareFill";
    return "square";
  }
  _row(l, depth, selIds) {
    const isFrame = l.itemType === "frame";
    const canReorder = !isFrame && this.app.orderBottomToTop().includes(l.id);
    const selected = isFrame ? this.app.selectedFrameId === l.id : selIds.includes(l.id);
    return html`
      <div class="row ${selected ? "sel" : ""} ${this._dragId === l.id ? "dragging" : ""} ${this._dropId === l.id ? "drop" : ""}"
        draggable=${canReorder ? "true" : "false"}
        style="padding-left:${6 + depth * 16}px"
        @dragstart=${(e) => { if (!canReorder) return; this._dragId = l.id; e.dataTransfer.effectAllowed = "move"; e.dataTransfer.setData("text/plain", String(l.id)); }}
        @dragover=${(e) => { if (!canReorder || this._dragId == null || this._dragId === l.id) return; e.preventDefault(); this._dropId = l.id; }}
        @dragleave=${() => { if (this._dropId === l.id) this._dropId = null; }}
        @drop=${(e) => { e.preventDefault(); if (canReorder) this._dropOn(l.id); }}
        @dragend=${() => { this._dragId = null; this._dropId = null; }}
        @contextmenu=${(e) => this._openContext(e, l, depth)}
        @click=${(e) => isFrame ? this.app.selectFrame(l.id) : (e.shiftKey ? this.app.toggleSelect(l.id) : this.app.select(l.id))}>
        <span class="ord">
          ${canReorder ? html`
            <button title="위로" @click=${(e) => { e.stopPropagation(); this.app.raise(l.id); }}>${icon("chevUpS", 9)}</button>
            <button title="아래로" @click=${(e) => { e.stopPropagation(); this.app.lower(l.id); }}>${icon("chevDownS", 9)}</button>` : nothing}
        </span>
        <span class="tic">${icon(this._layerIcon(l), 11)}</span>
        <span class="name" title=${isFrame ? `(${l.x},${l.y}) ${l.w}x${l.h}` : ""}
          @dblclick=${(e) => { if (!isFrame) { e.stopPropagation(); this._editing = l.id; } }}>
          ${!isFrame && this._editing === l.id
            ? html`<input .value=${l.name} autofocus
                @click=${(e) => e.stopPropagation()}
                @keydown=${(e) => { if (e.key === "Enter") this._rename(l.id, e.target.value); if (e.key === "Escape") this._editing = null; e.stopPropagation(); }}
                @blur=${(e) => this._rename(l.id, e.target.value)} />`
            : html`${l.name}${!isFrame && l.kind === "group" ? html` <span style="color:var(--fg-3)">(${l.children?.length ?? 0})</span>` : nothing}`}
        </span>
        ${isFrame ? html`
          <button class="b" title="PNG export"
            @click=${(e) => { e.stopPropagation(); this.app.exportFrame(l); }}>${icon("download", 13)}</button>
          <button class="b danger" title="프레임 제거"
            @click=${(e) => { e.stopPropagation(); this.app.removeFrame(l.id); }}>${icon("trash", 13)}</button>
        ` : html`
          <button class="b" title="표시/숨김"
            @click=${(e) => { e.stopPropagation(); this.app.apply([B.setProps(l.id, { visible: !l.visible })]); }}>
            ${icon(l.visible ? "eye" : "eyeOff", 13)}</button>
          <button class="b danger" title="삭제"
            @click=${(e) => { e.stopPropagation(); this.app.deleteMany([l.id]); }}>${icon("trash", 13)}</button>
        `}
      </div>
      ${(l.children ?? []).slice().reverse().map((child) => this._row(child, depth + 1, selIds))}
    `;
  }
  render() {
    const items = this._filterItems(this._items(), this._query);
    const selIds = this.app?.selectedIds ?? [];
    const projectName = new URLSearchParams(location.search).get("doc") || "Untitled";
    return html`
      <div class="project">
        <div class="project-name" title=${projectName}>${projectName}</div>
        <button class="add" title="프로젝트 관리"
          @click=${(e) => { e.stopPropagation(); this._projectMenu = !this._projectMenu; }}>
          ${icon("chevDown", 13)}
        </button>
        ${this._projectMenu ? html`
          <div class="project-menu">
            <button @click=${() => this._projectAction(() => { location.href = "/"; })}>${icon("chevLeft", 13)}대시보드</button>
            <button @click=${() => this._projectAction(() => this._renameProject(projectName))}>${icon("text", 13)}이름 변경</button>
            <div class="hr"></div>
            <button @click=${() => this._projectAction(() => this.dispatchEvent(new CustomEvent("export-png", { bubbles: true, composed: true })))}>${icon("export", 13)}전체 PNG</button>
            <button @click=${() => this._projectAction(() => this.dispatchEvent(new CustomEvent("save-dxpkg", { bubbles: true, composed: true })))}>${icon("save", 13)}.dxpkg 저장</button>
          </div>` : nothing}
      </div>
      <div class="head">
        <span class="head-title">레이어</span>
        <input class="search" type="search" placeholder="검색" .value=${this._query}
          @input=${(e) => { this._query = e.target.value; }} />
        <div class="head-actions">
          <button class="add" title="레이어 추가" @click=${(e) => { e.stopPropagation(); this._menu = !this._menu; }}>${icon("plus")}</button>
        </div>
      </div>
      <div class="list">
        ${this._menu ? html`
          <div class="menu">
            <button @click=${() => this._addLayer(null)}>${icon("square")}빈 레이어</button>
            <button @click=${() => this._addLayer([255, 255, 255, 255])}>${icon("squareFill")}단색 레이어</button>
            <button @click=${() => this.renderRoot.querySelector("#png2").click()}>${icon("image")}이미지 가져오기</button>
            <input id="png2" type="file" accept="image/png" style="display:none" @change=${(e) => this._addPng(e)} />
          </div>` : nothing}
        ${items.length === 0 ? html`<div class="empty">레이어가 없습니다.<br>도형을 그리거나 dx CLI로 추가하세요.</div>` : nothing}
        ${items.map((l) => this._row(l, 0, selIds))}
        ${this._ctx ? html`
          <div class="ctx" style="left:${this._ctx.x}px; top:${this._ctx.y}px">
            ${this._ctx.kind === "frame" ? (() => {
              const f = this.app.frames().find((v) => v.id === this._ctx.id);
              return html`
                <button @click=${() => this._menuAction(() => f && this.app.exportFrame(f))}>${icon("download", 13)}프레임 PNG</button>
                <button @click=${() => this._menuAction(() => this.app.removeFrame(this._ctx.id))}>${icon("trash", 13)}프레임 삭제</button>`;
            })() : html`
              <button @click=${() => this._menuAction(() => this.app.duplicateMany(this.app.selectedIds))}>${icon("dup", 13)}복제</button>
              <button ?disabled=${selIds.length < 2} @click=${() => this._menuAction(() => this.app.groupSelected())}>${icon("folder", 13)}그룹</button>
              <button ?disabled=${this.app.getSelected()?.kind !== "group"} @click=${() => this._menuAction(() => this.app.ungroupSelected())}>${icon("dup", 13)}그룹 해제</button>
              <div class="hr"></div>
              <button ?disabled=${!this.app.orderBottomToTop().includes(this._ctx.id)} @click=${() => this._menuAction(() => this.app.raiseMany(this.app.selectedIds))}>${icon("chevUpS", 13)}앞으로</button>
              <button ?disabled=${!this.app.orderBottomToTop().includes(this._ctx.id)} @click=${() => this._menuAction(() => this.app.lowerMany(this.app.selectedIds))}>${icon("chevDownS", 13)}뒤로</button>
              <div class="hr"></div>
              <button @click=${() => this._menuAction(() => this.app.deleteMany(this.app.selectedIds))}>${icon("trash", 13)}삭제</button>
            `}
          </div>` : nothing}
      </div>
    `;
  }
}
customElements.define("dx-layer-panel", DxLayerPanel);

// ───────── Design 패널 (우측) ─────────
class DxProps extends LitElement {
  static properties = { app: { attribute: false }, _v: { state: true } };
  static styles = [controls, css`
    :host {
      display: block; position: fixed; right: 24px; top: 108px; z-index: 70;
      width: 244px; max-height: calc(100vh - 158px);
      background: var(--bg-panel); border: 1px solid var(--line);
      border-radius: 10px; overflow-y: auto;
      box-shadow: 0 14px 38px rgba(0, 0, 0, 0.32);
    }
    .head {
      padding: 12px; font-size: 11px; font-weight: 600; color: var(--fg);
      border-bottom: 1px solid var(--line); display: flex; align-items: center; gap: 6px;
    }
    .head .nm { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--fg-2); font-weight: 400; }
    .head .b { width: 26px; height: 26px; padding: 0; justify-content: center; }
    .sec { padding: 12px; border-bottom: 1px solid var(--line-soft); }
    .sec-t { font-size: 10.5px; font-weight: 600; color: var(--fg); margin-bottom: 10px; }
    .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: 7px; }
    .cell { display: flex; align-items: center; gap: 0; background: var(--bg-elev); border-radius: var(--radius); border: 1px solid transparent; }
    .cell:focus-within { border-color: var(--accent); }
    .cell span { padding: 0 0 0 7px; color: var(--fg-3); font-size: 10px; }
    .cell input { background: none; border: none; width: 100%; padding: 0 6px 0 4px; }
    .alignr { display: flex; gap: 1px; justify-content: space-between; }
    .alignr button { width: 32px; height: 28px; padding: 0; justify-content: center; }
    .field { margin-top: 10px; }
    .field:first-child { margin-top: 0; }
    .field > label { display: flex; justify-content: space-between; align-items: center; font-size: 10.5px; color: var(--fg-2); margin-bottom: 6px; }
    .field .v { color: var(--fg); }
    .field input[type="range"], .field select { width: 100%; }
    .chk { display: flex; align-items: center; gap: 8px; font-size: 11px; color: var(--fg-2); cursor: pointer; }
    .empty { padding: 22px 14px; font-size: 11px; color: var(--fg-3); line-height: 1.8; }
  `];
  constructor() { super(); this._v = 0; }
  connectedCallback() { super.connectedCallback(); this._onChange = () => { this._v++; }; this.app?.addEventListener("changed", this._onChange); }
  disconnectedCallback() { this.app?.removeEventListener("changed", this._onChange); super.disconnectedCallback(); }
  _set(patch) { this.app.apply([B.setProps(this.app.selectedId, patch)]); }
  render() {
    const ids = this.app?.selectedIds ?? [];
    const frame = this.app?.getSelectedFrame?.();
    this.style.display = (ids.length || frame) ? "block" : "none";
    if (frame) {
      const num = (label, value, onChange) => html`
        <div class="cell"><span>${label}</span>
          <input type="number" .value=${String(value)} @change=${(e) => onChange(+e.target.value || 0)} /></div>`;
      return html`
        <div class="head">Frame<span class="nm">· ${frame.name}</span>
          <button class="b" title="PNG export" @click=${() => this.app.exportFrame(frame)}>${icon("download", 13)}</button>
          <button class="b" title="삭제" @click=${() => this.app.removeFrame(frame.id)}>${icon("trash", 13)}</button>
        </div>
        <div class="sec">
          <div class="sec-t">이름</div>
          <input style="width:100%" .value=${frame.name}
            @change=${(e) => this.app.updateFrame(frame.id, { name: e.target.value.trim() || frame.name })} />
        </div>
        <div class="sec">
          <div class="sec-t">위치 · 크기</div>
          <div class="grid2">
            ${num("X", frame.x, (v) => this.app.updateFrame(frame.id, { x: Math.round(v) }))}
            ${num("Y", frame.y, (v) => this.app.updateFrame(frame.id, { y: Math.round(v) }))}
            ${num("W", frame.w, (v) => this.app.updateFrame(frame.id, { w: Math.max(1, Math.round(v)) }))}
            ${num("H", frame.h, (v) => this.app.updateFrame(frame.id, { h: Math.max(1, Math.round(v)) }))}
          </div>
        </div>
        <div class="sec">
          <button style="width:100%; justify-content:center" @click=${() => this.app.exportFrame(frame)}>
            ${icon("download", 13)}프레임 PNG 내보내기
          </button>
        </div>`;
    }
    // 다중 선택: "N개 선택됨" + 공통 액션(정렬·삭제)만 표시.
    if (ids.length > 1) {
      return html`
        <div class="head">Design<span class="nm">· ${ids.length}개 선택됨</span>
          <button class="b" title="삭제 (Del)" @click=${() => this.app.deleteMany(ids)}>${icon("trash", 13)}</button>
        </div>
        <div class="sec">
          <div class="sec-t">정렬</div>
          <div class="alignr">
            ${["left", "center-h", "right", "top", "center-v", "bottom"].map((m, i) => html`
              <button title=${m} @click=${() => this.app.alignMany(ids, m)}>
                ${icon(["alignL", "alignCH", "alignR", "alignT", "alignCV", "alignB"][i], 14)}</button>`)}
          </div>
        </div>
        <div class="empty">${ids.length}개 레이어가 선택되었습니다.<br>개별 속성은 단일 선택에서 편집하세요.</div>`;
    }
    const l = this.app?.getSelected?.();
    if (!l) return html``;
    const [ox, oy] = l.offset ?? [0, 0];
    const [sx, sy] = l.scale ?? [1, 1];
    const b = this.app.layerBounds(l.id);
    const wPx = b ? Math.round(b[2] * Math.abs(sx)) : 0;
    const hPx = b ? Math.round(b[3] * Math.abs(sy)) : 0;
    // W/H 편집: 도형 좌상단 고정(anchor) — 위치가 같이 움직이지 않게 offset 보정.
    const tlAnchor = b ? { x: b[0], y: b[1] } : null;
    const ctrAnchor = b ? { x: b[0] + b[2] / 2, y: b[1] + b[3] / 2 } : null;
    const setScaleAnchored = (ns, anchor) => {
      const off = this.app.computeAnchoredOffset(l, ns, null, anchor);
      this.app.apply([B.setProps(l.id, { scale: ns, offset: off })]);
    };
    const setW = (v) => { if (b && b[2] > 0 && v > 0) setScaleAnchored([(v / b[2]) * Math.sign(sx || 1), sy], tlAnchor); };
    const setH = (v) => { if (b && b[3] > 0 && v > 0) setScaleAnchored([sx, (v / b[3]) * Math.sign(sy || 1)], tlAnchor); };
    const setRotAnchored = (deg) => {
      const off = this.app.computeAnchoredOffset(l, null, deg, ctrAnchor ?? { x: 0, y: 0 });
      this.app.apply([B.setProps(l.id, { rotation: deg, offset: off })]);
    };
    // X/Y는 절대좌표: 문서 좌상단 = (0,0), 값 = 오브젝트(변환 후 AABB) 좌상단.
    // 편집 시 목표 절대좌표와 현재 AABB 차이만큼 offset 이동.
    const aabb = this.app.displayAABB(l);
    const absX = aabb ? Math.round(aabb.x) : ox;
    const absY = aabb ? Math.round(aabb.y) : oy;
    const commitXY = (which, v) => {
      if (!aabb) return;
      const dx = which === "x" ? Math.round(+v - aabb.x) : 0;
      const dy = which === "y" ? Math.round(+v - aabb.y) : 0;
      this.app.apply([B.setOffset(l.id, [ox + dx, oy + dy])]);
    };
    const num = (label, value, onChange) => html`
      <div class="cell"><span>${label}</span>
        <input type="number" .value=${String(value)} @change=${(e) => onChange(e.target.value)} /></div>`;
    return html`
      <div class="head">Design<span class="nm">· ${l.name}</span>
        <button class="b" title="복제 (Cmd+D)" @click=${() => this.app.duplicate(l.id)}>${icon("dup", 13)}</button>
        <button class="b" title="삭제 (Del)" @click=${() => this.app.apply([B.deleteLayer(l.id)])}>${icon("trash", 13)}</button>
      </div>
      <div class="sec">
        <div class="sec-t">위치 · 크기</div>
        <div class="grid2">
          ${num("X", absX, (v) => commitXY("x", v))}
          ${num("Y", absY, (v) => commitXY("y", v))}
          ${num("W", wPx, (v) => setW(+v))}
          ${num("H", hPx, (v) => setH(+v))}
          ${num("R°", Math.round((l.rotation ?? 0) * 10) / 10, (v) => setRotAnchored(+v || 0))}
          <div class="cell"><span>S</span>
            <input type="text" .value=${`${sx} , ${sy}`} title="scale (x , y)"
              @change=${(e) => {
                const m = e.target.value.split(",").map((s2) => parseFloat(s2));
                if (m.length === 2 && m.every((n) => Number.isFinite(n) && n !== 0))
                  setScaleAnchored(m, ctrAnchor ?? { x: 0, y: 0 });
              }} /></div>
        </div>
      </div>
      <div class="sec">
        <div class="sec-t">정렬</div>
        <div class="alignr">
          ${["left", "center-h", "right", "top", "center-v", "bottom"].map((m, i) => html`
            <button title=${m} @click=${() => this.app.align(l.id, m)}>
              ${icon(["alignL", "alignCH", "alignR", "alignT", "alignCV", "alignB"][i], 14)}</button>`)}
        </div>
      </div>
      <div class="sec">
        <div class="sec-t">속성</div>
        <div class="field">
          <label>불투명도 <span class="v">${Math.round(l.opacity * 100)}%</span></label>
          <input type="range" min="0" max="1" step="0.01" .value=${String(l.opacity)}
            @input=${(e) => this._set({ opacity: +e.target.value })} />
        </div>
        <div class="field">
          <label>블렌드</label>
          <select .value=${l.blend} @change=${(e) => this.app.apply([B.setBlend(l.id, e.target.value)])}>
            <option value="normal">Normal</option>
            <option value="multiply">Multiply</option>
            <option value="screen">Screen</option>
            <option value="darken">Darken</option>
            <option value="lighten">Lighten</option>
            <option value="overlay">Overlay</option>
            <option value="difference">Difference</option>
          </select>
        </div>
        <div class="field">
          <label class="chk"><input type="checkbox" .checked=${l.visible}
            @change=${(e) => this._set({ visible: e.target.checked })} /> 표시</label>
        </div>
      </div>
    `;
  }
}
customElements.define("dx-props", DxProps);

// ───────── 앱 셸 ─────────
class AppShell extends LitElement {
  static properties = { app: { attribute: false }, _tool: { state: true }, _zoom: { state: true }, _theme: { state: true } };
  static styles = css`
    :host {
      display: grid; height: 100vh;
      grid-template-columns: 240px 1fr;
      grid-template-areas: "layers canvas";
      background: var(--bg-canvas);
    }
  `;
  constructor() {
    super();
    this._tool = null; this._zoom = 1;
    this._theme = localStorage.getItem("dx-theme") || "dark";
    document.documentElement.dataset.theme = this._theme;
  }
  connectedCallback() {
    super.connectedCallback();
    window.addEventListener("keydown", (this._key = (e) => this._onKey(e)));
  }
  disconnectedCallback() { window.removeEventListener("keydown", this._key); super.disconnectedCallback(); }
  get _topbar() { return this.renderRoot?.querySelector("dx-topbar"); }
  get _canvas() { return this.renderRoot?.querySelector("dx-canvas"); }
  _onKey(e) {
    const t = e.composedPath?.()[0] ?? e.target;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT")) return;
    if (e.isComposing) return;
    const meta = e.metaKey || e.ctrlKey;
    const ids = this.app?.selectedIds ?? []; // 단축키는 선택 전체에 적용(하나의 apply 배치).
    const k = e.key.toLowerCase();
    if (meta && k === "z") { e.preventDefault(); e.shiftKey ? this.app.redo() : this.app.undo(); return; }
    if (meta && k === "g") { e.preventDefault(); e.shiftKey ? this.app.ungroupSelected() : this.app.groupSelected(); return; }
    if (meta && k === "d") { e.preventDefault(); this.app.duplicateMany(ids); return; }
    // 내부 클립보드: Cmd+C = 선택 id 기억, Cmd+V = 살아있는 것만 복제.
    if (meta && k === "c") {
      if (ids.length) { e.preventDefault(); this.app.copy(ids); }
      return;
    }
    if (meta && k === "v") {
      if (this.app.clipboardIds?.length) { e.preventDefault(); this.app.paste(); }
      return;
    }
    // Shift+H/V = 선택 레이어 좌우/상하 뒤집기(선택 없으면 도구 단축키로 폴스루).
    if (!meta && e.shiftKey && ids.length && (k === "h" || k === "v")) {
      e.preventDefault();
      this.app.flipMany(ids, k === "h" ? "x" : "y");
      return;
    }
    if (meta && (e.key === "]" || e.key === "[")) {
      e.preventDefault();
      if (ids.length) e.key === "]" ? this.app.raiseMany(ids) : this.app.lowerMany(ids);
      return;
    }
    if (!meta && (e.key === "Delete" || e.key === "Backspace")) {
      if (ids.length) { e.preventDefault(); this.app.deleteMany(ids); this.app.select(null); }
      return;
    }
    if (!meta && e.key.startsWith("Arrow")) {
      if (!ids.length) return;
      e.preventDefault();
      const d = e.shiftKey ? 10 : 1;
      const dx = e.key === "ArrowLeft" ? -d : e.key === "ArrowRight" ? d : 0;
      const dy = e.key === "ArrowUp" ? -d : e.key === "ArrowDown" ? d : 0;
      this.app.nudgeMany(ids, dx, dy);
      return;
    }
    if (e.shiftKey && e.key === "0") { this._canvas?.zoomCmd("reset"); return; }
    if (e.shiftKey && e.key === "1") { this._canvas?.zoomCmd("fit"); return; }
    if (!meta) {
      const map = { v: "select", r: "rect", e: "ellipse", l: "line", t: "text", b: "brush", f: "frame" };
      if (map[k]) { this._topbar?.setTool(map[k]); return; }
      if (e.key === "Escape") this.app.select(null);
    }
  }
  _toggleTheme() {
    this._theme = this._theme === "dark" ? "light" : "dark";
    document.documentElement.dataset.theme = this._theme;
    localStorage.setItem("dx-theme", this._theme);
  }
  render() {
    if (!this.app) return html`<div style="padding:40px;color:var(--fg-3)">loading…</div>`;
    return html`
      <dx-topbar .app=${this.app} .zoom=${this._zoom} .theme=${this._theme}
        @tool-changed=${(e) => { this._tool = e.detail; }}
        @zoom-cmd=${(e) => this._canvas?.zoomCmd(e.detail)}
        @theme-toggle=${() => this._toggleTheme()}
        @export-png=${() => this.dispatchEvent(new CustomEvent("export-png", { bubbles: true }))}
        @save-dxpkg=${() => this.dispatchEvent(new CustomEvent("save-dxpkg", { bubbles: true }))}></dx-topbar>
      <dx-layer-panel .app=${this.app}
        @export-png=${() => this.dispatchEvent(new CustomEvent("export-png", { bubbles: true }))}
        @save-dxpkg=${() => this.dispatchEvent(new CustomEvent("save-dxpkg", { bubbles: true }))}></dx-layer-panel>
      <dx-canvas .app=${this.app} .toolState=${this._tool}
        @zoom-changed=${(e) => { this._zoom = e.detail; }}
        @picked-color=${(e) => { this._topbar?.setColor(e.detail); this._topbar?.finishEyedrop(); }}
        @text-finished=${() => this._topbar?.setTool("select")}></dx-canvas>
      <dx-props .app=${this.app}></dx-props>
    `;
  }
}
customElements.define("app-shell", AppShell);
