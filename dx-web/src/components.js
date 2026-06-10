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
const needsWidth = (t) => t === "line" || t === "stroke-rect" || t === "stroke-ellipse";

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

// ───────── 상단 툴바 ─────────
class DxTopbar extends LitElement {
  static properties = {
    app: { attribute: false }, zoom: { attribute: false }, theme: { attribute: false },
    tool: { state: true }, color: { state: true }, alpha: { state: true },
    width: { state: true }, radius: { state: true }, fontSize: { state: true },
    _shape: { state: true }, _menu: { state: true }, _v: { state: true },
  };
  static styles = [controls, css`
    :host {
      grid-area: topbar; display: flex; align-items: center; gap: 2px;
      height: 44px; padding: 0 8px; background: var(--bg-panel);
      border-bottom: 1px solid var(--line);
    }
    .logo {
      font-weight: 600; font-size: 12.5px; color: var(--fg); letter-spacing: 0.2px;
      padding: 0 10px 0 6px; display: flex; align-items: center; gap: 7px;
    }
    .logo .dot { width: 8px; height: 8px; border-radius: 2px; background: var(--accent); }
    .tools { display: flex; gap: 1px; align-items: center; position: relative; }
    .tools button { width: 34px; height: 34px; padding: 0; justify-content: center; }
    .tools button.dd { width: 44px; gap: 1px; }
    .sep { width: 1px; height: 20px; background: var(--line); margin: 0 7px; }
    .menu {
      position: absolute; left: 36px; top: 40px; z-index: 50; background: var(--bg-panel);
      border: 1px solid var(--line); border-radius: 9px; padding: 5px; min-width: 170px;
      box-shadow: var(--shadow-menu); display: flex; flex-direction: column; gap: 1px;
    }
    .menu button { width: 100%; justify-content: flex-start; height: 30px; color: var(--fg); }
    .menu button:hover { background: var(--accent); color: #fff; }
    .menu .key { margin-left: auto; color: inherit; opacity: 0.5; font-size: 10px; }
    .opts { display: flex; align-items: center; gap: 8px; margin-left: 4px; }
    .swatch { width: 24px; height: 24px; border: 1px solid var(--line); border-radius: 5px;
              padding: 0; cursor: pointer; overflow: hidden; position: relative; flex: none; }
    .swatch input { position: absolute; inset: -4px; width: 36px; height: 36px; border: none; padding: 0; cursor: pointer; }
    .opts label { display: flex; align-items: center; gap: 5px; color: var(--fg-3); font-size: 10.5px; }
    .opts input[type="range"] { width: 64px; }
    .opts .num { width: 44px; text-align: right; }
    .spacer { flex: 1; }
    .doc { color: var(--fg-2); font-size: 11px; margin-right: 6px; }
    .zoom { display: flex; align-items: center; gap: 0; }
    .zoom .pct { min-width: 48px; text-align: center; color: var(--fg-2); cursor: pointer; }
    .ico { width: 30px; padding: 0; justify-content: center; }
  `];
  constructor() {
    super();
    this.tool = "select"; this.color = "#0d99ff"; this.alpha = 1;
    this.width = 4; this.radius = 12; this.fontSize = 32;
    this._shape = "rect"; this._menu = false; this._v = 0;
    this.zoom = 1; this.theme = "dark";
  }
  connectedCallback() {
    super.connectedCallback();
    this._onChange = () => { this._v++; };
    this.app?.addEventListener("changed", this._onChange);
    this._onDoc = (e) => { if (this._menu && !e.composedPath().includes(this)) this._menu = false; };
    document.addEventListener("click", this._onDoc);
  }
  disconnectedCallback() {
    this.app?.removeEventListener("changed", this._onChange);
    document.removeEventListener("click", this._onDoc);
    super.disconnectedCallback();
  }
  /** 외부(단축키/스포이드)에서 도구·색 설정. */
  setTool(t) { this._pick(t); }
  setColor(rgba) { this.color = HEX(rgba); this._emit(); }
  _pick(t) {
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
      this.app?.apply([B.addPaintLayer(file.name.replace(/\.[^.]+$/, ""), B.pngBase64(b64))]);
    };
    reader.readAsDataURL(file);
    e.target.value = "";
  }
  render() {
    const cur = SHAPES.find((s) => s.id === this._shape) ?? SHAPES[0];
    const isDraw = this.tool !== "select" && this.tool !== "eyedrop";
    const docId = new URLSearchParams(location.search).get("doc");
    const t = (id, ic, title) => html`<button class=${this.tool === id ? "active" : ""}
      title=${title} @click=${() => this._pick(id)}>${icon(ic)}</button>`;
    return html`
      <span class="logo"><span class="dot"></span>DesignCLI</span>
      <div class="tools">
        ${t("select", "cursor", "선택/이동 (V)")}
        <button class="dd ${isShapeTool(this.tool) ? "active" : ""}" title="도형 (${cur.label})"
          @click=${() => { this._menu = !this._menu; }}>${icon(cur.ic)}${icon("chevDown", 9)}</button>
        ${t("line", "line", "선 (L)")}
        ${t("text", "text", "텍스트 (T)")}
        <button title="이미지(PNG) 레이어" @click=${() => this.renderRoot.querySelector("#png").click()}>${icon("image")}</button>
        ${t("eyedrop", "dropper", "스포이드 (I)")}
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
          <label>A<input type="range" min="0" max="1" step="0.05" .value=${String(this.alpha)}
            @input=${(e) => { this.alpha = +e.target.value; this._emit(); }} /></label>
          ${needsWidth(this.tool) ? html`<label>W<input class="num" type="number" min="1" max="100" .value=${String(this.width)}
            @change=${(e) => { this.width = +e.target.value || 1; this._emit(); }} /></label>` : nothing}
          ${this.tool === "rounded-rect" ? html`<label>R<input class="num" type="number" min="0" max="200" .value=${String(this.radius)}
            @change=${(e) => { this.radius = +e.target.value || 0; this._emit(); }} /></label>` : nothing}
          ${this.tool === "text" ? html`<label>크기<input class="num" type="number" min="6" max="400" .value=${String(this.fontSize)}
            @change=${(e) => { this.fontSize = +e.target.value || 12; this._emit(); }} /></label>` : nothing}
        </div>` : nothing}
      <span class="spacer"></span>
      <span class="doc">${docId ? `${docId} · live` : "local"}</span>
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
      <button class="ico" title="테마 전환"
        @click=${() => this.dispatchEvent(new CustomEvent("theme-toggle", { bubbles: true, composed: true }))}>
        ${icon(this.theme === "dark" ? "sun" : "moon")}</button>
      <button title="PNG 내보내기" @click=${() => this.dispatchEvent(new CustomEvent("export-png", { bubbles: true, composed: true }))}>${icon("export")}Export</button>
      <button title=".dxpkg 저장" @click=${() => this.dispatchEvent(new CustomEvent("save-dxpkg", { bubbles: true, composed: true }))}>${icon("save")}</button>
    `;
  }
}
customElements.define("dx-topbar", DxTopbar);

// ───────── 캔버스 (줌/팬 + 선택/이동/리사이즈/회전 + 그리기 + 텍스트 + 스포이드) ─────────
const HANDLE = 8; // 핸들 화면 px
class DxCanvas extends LitElement {
  static properties = { app: { attribute: false }, toolState: { attribute: false }, _v: { state: true }, _text: { state: true } };
  static styles = css`
    :host { grid-area: canvas; position: relative; display: block; overflow: auto; background: var(--bg-canvas); }
    .wrap { position: relative; margin: 48px; width: fit-content; }
    canvas { display: block; }
    #base { box-shadow: 0 0 0 1px var(--line), 0 8px 30px rgba(0,0,0,0.35); background:
      repeating-conic-gradient(rgba(127,127,127,0.12) 0% 25%, transparent 0% 50%) 50% / 16px 16px; }
    #overlay { position: absolute; left: 0; top: 0; pointer-events: none; }
    textarea.txt {
      position: absolute; z-index: 20; background: transparent; color: var(--fg);
      border: 1.5px solid var(--accent); border-radius: 2px; outline: none; resize: none;
      font-family: "Pretendard", "Inter", sans-serif; line-height: 1.25; padding: 0; overflow: hidden;
      min-width: 40px; min-height: 1em; white-space: pre;
    }
  `;
  constructor() {
    super();
    this._drag = null; this._v = 0; this._zoom = 1; this._space = false; this._text = null;
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
  }
  disconnectedCallback() {
    this.app?.removeEventListener("changed", this._onChange);
    window.removeEventListener("pointermove", this._mv);
    window.removeEventListener("pointerup", this._up);
    window.removeEventListener("keydown", this._kd);
    window.removeEventListener("keyup", this._ku);
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
    this.app.renderer.resize();
    this._applyZoom();
    this.base.addEventListener("pointerdown", (e) => this._down(e));
    this.base.addEventListener("dblclick", (e) => this._dblclick(e));
    this.addEventListener("wheel", (e) => this._wheel(e), { passive: false });
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
    if ((this.toolState?.tool ?? "select") !== "select") return;
    const p = this._coords(e);
    const hit = this.app.hitTest(p.x, p.y);
    if (hit == null) return;
    const layer = this.app.layers().find((l) => l.id === hit);
    let meta = null;
    try { meta = layer?.meta ? JSON.parse(layer.meta) : null; } catch { /* meta 없음 */ }
    if (meta?.type !== "text") return;
    e.preventDefault();
    this.app.select(hit);
    this._openText({
      x: meta.x, y: meta.y, value: meta.text,
      size: meta.size, rgba: meta.rgba,
      editId: hit,
    });
  }
  updated() { this._applyZoom(); this._drawOverlay(); }

  // ---- 줌/팬 ----
  get zoom() { return this._zoom; }
  _applyZoom() {
    if (!this.base) return;
    const W = this.app.editor.width(), H = this.app.editor.height();
    const z = this._zoom;
    this.base.style.width = `${W * z}px`;
    this.base.style.height = `${H * z}px`;
    this.base.style.imageRendering = z >= 1 ? "pixelated" : "auto";
    const ow = Math.max(1, Math.round(W * z)), oh = Math.max(1, Math.round(H * z));
    if (this.overlay.width !== ow) this.overlay.width = ow;
    if (this.overlay.height !== oh) this.overlay.height = oh;
    this.overlay.style.width = `${W * z}px`;
    this.overlay.style.height = `${H * z}px`;
  }
  _setZoom(z, cx, cy) {
    const old = this._zoom;
    z = Math.min(8, Math.max(0.05, z));
    if (z === old) return;
    // 포인터 위치 고정 줌: content 좌표 보존.
    const rect = this.getBoundingClientRect();
    const px = (cx ?? rect.left + rect.width / 2) - rect.left;
    const py = (cy ?? rect.top + rect.height / 2) - rect.top;
    const k = z / old;
    const sl = (this.scrollLeft + px) * k - px;
    const st = (this.scrollTop + py) * k - py;
    this._zoom = z;
    this._applyZoom();
    this.scrollLeft = sl;
    this.scrollTop = st;
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
      this._setZoom(z);
    }
  }
  _wheel(e) {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      this._setZoom(this._zoom * Math.exp(-e.deltaY * 0.01), e.clientX, e.clientY);
    }
  }

  // ---- 좌표 ----
  _coords(e) {
    const r = this.base.getBoundingClientRect();
    return {
      x: (e.clientX - r.left) * (this.base.width / r.width),
      y: (e.clientY - r.top) * (this.base.height / r.height),
    };
  }

  // ---- 핸들 (화면 좌표 계산) ----
  /** 레이어 1개의 선택 지오메트리(드래그 중 임시값 미리보기 반영). */
  _selGeomFor(sel) {
    const b = this.app.layerBounds(sel.id);
    if (!b) return null;
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
    ].map((p) => ({ x: p.x * z, y: p.y * z })); // 시계방향 TL,TR,BR,BL (화면px)
    const mid = (a, b2) => ({ x: (a.x + b2.x) / 2, y: (a.y + b2.y) / 2 });
    const handles = [
      { k: "tl", p: c4[0], ax: "xy" }, { k: "tm", p: mid(c4[0], c4[1]), ax: "y" },
      { k: "tr", p: c4[1], ax: "xy" }, { k: "mr", p: mid(c4[1], c4[2]), ax: "x" },
      { k: "br", p: c4[2], ax: "xy" }, { k: "bm", p: mid(c4[2], c4[3]), ax: "y" },
      { k: "bl", p: c4[3], ax: "xy" }, { k: "ml", p: mid(c4[3], c4[0]), ax: "x" },
    ];
    const ctr = tp.center();
    const ctrS = { x: ctr.x * z, y: ctr.y * z };
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
    const r = this.base.getBoundingClientRect();
    const sx = e.clientX - r.left, sy = e.clientY - r.top;
    const near = (p, rad) => Math.hypot(p.x - sx, p.y - sy) <= rad;
    if (near(g.rot, HANDLE)) return { type: "rotate", g };
    for (const h of g.handles) if (near(h.p, HANDLE)) return { type: "resize", h, g };
    return null;
  }

  // ---- 포인터 ----
  _down(e) {
    if (this._space) {
      this._drag = { mode: "pan", sx: e.clientX, sy: e.clientY, sl: this.scrollLeft, st: this.scrollTop };
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
        const c = g.t.c;
        // 핸들의 src 기준점(코너/엣지 중점) — 축별로 중심에서의 거리로 스케일 계산.
        const hx = hh.h.k.includes("l") ? b[0] : hh.h.k.includes("r") ? b[0] + b[2] : b[0] + b[2] / 2;
        const hy = hh.h.k.startsWith("t") ? b[1] : hh.h.k.startsWith("b") ? b[1] + b[3] : b[1] + b[3] / 2;
        // anchor = 반대쪽 핸들(src) — Figma처럼 반대편이 고정된 채 늘어난다.
        const ax2 = hh.h.k.includes("l") ? b[0] + b[2] : hh.h.k.includes("r") ? b[0] : b[0] + b[2] / 2;
        const ay2 = hh.h.k.startsWith("t") ? b[1] + b[3] : hh.h.k.startsWith("b") ? b[1] : b[1] + b[3] / 2;
        this._drag = {
          mode: "resize", id: sel.id, ax: hh.h.ax,
          scale0: sel.scale ?? [1, 1], provScale: sel.scale ?? [1, 1], provOffset: null,
          hsrc: { x: hx, y: hy }, asrc: { x: ax2, y: ay2 }, c, sel,
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
        // 빈 공간 드래그 = 마퀴 다중선택(클릭이면 해제 — _end에서 판정).
        this._drag = { mode: "marquee", start: p, cur: p };
        return;
      }
      // 이미 선택된 레이어를 잡으면 선택 유지(다중 함께 이동), 아니면 단일 교체.
      if (!this.app.selectedIds.includes(hit)) this.app.select(hit);
      const all = this.app.layers();
      const bases = new Map();
      for (const id of this.app.selectedIds) {
        const l = all.find((v) => v.id === id);
        if (l) bases.set(id, l.offset ?? [0, 0]);
      }
      this._drag = { mode: "move", ids: [...this.app.selectedIds], bases, start: p, dx: 0, dy: 0 };
      this.style.cursor = "grabbing";
      return;
    }
    // 그리기 도구
    this._drag = { mode: "draw", start: p, cur: p };
  }
  _move(e) {
    const d = this._drag;
    if (!d) return;
    if (d.mode === "pan") {
      this.scrollLeft = d.sl - (e.clientX - d.sx);
      this.scrollTop = d.st - (e.clientY - d.sy);
      return;
    }
    const p = this._coords(e);
    if (d.mode === "move") {
      d.dx = Math.round(p.x - d.start.x);
      d.dy = Math.round(p.y - d.start.y);
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
      // 회전 프레임 제거한 로컬 좌표에서 축별 비율.
      const rad = ((sel.rotation ?? 0) * Math.PI) / 180;
      const cos = Math.cos(rad), sin = Math.sin(rad);
      const ax0 = p.x - (sel.offset?.[0] ?? 0) - d.c.x;
      const ay0 = p.y - (sel.offset?.[1] ?? 0) - d.c.y;
      const lx = cos * ax0 + sin * ay0; // = sx' * (hsrc.x - c.x)
      const ly = -sin * ax0 + cos * ay0;
      let [nsx, nsy] = d.scale0;
      const bx = d.hsrc.x - d.c.x, by = d.hsrc.y - d.c.y;
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
    } else if (d.mode === "draw") {
      d.cur = p;
      this._drawGhost();
    }
  }
  _end() {
    const d = this._drag;
    if (!d) return;
    this._drag = null;
    this.style.cursor = this._space ? "grab" : "";
    if (d.mode === "pan") return;
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
    this.app.apply([B.addPaintLayer(name, B.shapes([shape]))]);
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
      const idx = this.app.orderBottomToTop().indexOf(t.editId);
      this.app.apply([
        B.deleteLayer(t.editId),
        B.addPaintLayer(name, B.shapes([B.text(t.x, t.y, v, size, rgba)]), { index: idx >= 0 ? idx : undefined, bind: "t" }),
        B.setProps("t", { meta }),
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
    const s = this.toolState; const [r, g, b, a] = s.rgba;
    o.strokeStyle = `rgba(${r},${g},${b},${a / 255})`;
    o.fillStyle = `rgba(${r},${g},${b},${(a / 255) * 0.45})`;
    o.lineWidth = 1; o.setLineDash([4, 3]);
    const { start, cur } = this._drag;
    const bx = Math.min(start.x, cur.x) * z, by = Math.min(start.y, cur.y) * z;
    const bw = Math.abs(cur.x - start.x) * z, bh = Math.abs(cur.y - start.y) * z;
    switch (s.tool) {
      case "rect": o.fillRect(bx, by, bw, bh); o.strokeRect(bx, by, bw, bh); break;
      case "ellipse": o.beginPath(); o.ellipse(bx + bw / 2, by + bh / 2, bw / 2, bh / 2, 0, 0, 7); o.fill(); o.stroke(); break;
      case "stroke-rect": o.setLineDash([]); o.lineWidth = s.width * z; o.strokeRect(bx, by, bw, bh); break;
      case "stroke-ellipse": o.setLineDash([]); o.lineWidth = s.width * z; o.beginPath(); o.ellipse(bx + bw / 2, by + bh / 2, bw / 2, bh / 2, 0, 0, 7); o.stroke(); break;
      case "rounded-rect": o.beginPath(); if (o.roundRect) o.roundRect(bx, by, bw, bh, s.radius * z); else o.rect(bx, by, bw, bh); o.fill(); o.stroke(); break;
      default: o.setLineDash([]); o.lineWidth = s.width * z; o.beginPath(); o.moveTo(start.x * z, start.y * z); o.lineTo(cur.x * z, cur.y * z); o.stroke();
    }
    o.setLineDash([]);
  }
  _drawOverlay() {
    if (!this.overlay) return;
    const o = this.overlay.getContext("2d");
    o.clearRect(0, 0, this.overlay.width, this.overlay.height);
    if (this._text) return; // 텍스트 편집 중엔 셀렉션 크롬 숨김(입력에 집중).
    if (this._drag?.mode === "draw") { this._drawGhost(); return; }
    const acc = getComputedStyle(this).getPropertyValue("--accent").trim() || "#0d99ff";
    // 마퀴(점선 사각형, --accent 색).
    if (this._drag?.mode === "marquee") {
      const d = this._drag, z = this._zoom;
      const x = Math.min(d.start.x, d.cur.x) * z, y = Math.min(d.start.y, d.cur.y) * z;
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
    for (const sel of this.app.selectedLayers?.() ?? []) {
      const g = this._selGeomFor(sel);
      if (!g) continue;
      o.beginPath();
      o.moveTo(g.c4[0].x, g.c4[0].y);
      for (let i = 1; i < 4; i++) o.lineTo(g.c4[i].x, g.c4[i].y);
      o.closePath();
      o.fillStyle = "rgba(13,153,255,0.08)";
      o.fill();
      o.strokeStyle = acc; o.lineWidth = 1.5;
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
  }
  render() {
    const z = this._zoom;
    const t = this._text;
    const s = this.toolState;
    return html`
      <div class="wrap">
        <canvas id="base"></canvas><canvas id="overlay"></canvas>
        ${t ? html`<textarea class="txt" spellcheck="false"
          style="left:${t.x * z}px; top:${t.y * z}px; font-size:${(t.size ?? s?.size ?? 32) * z}px; color:${HEX(t.rgba ?? s?.rgba ?? [13, 153, 255, 255])}"
          .value=${t.value}
          @input=${(e) => { t.value = e.target.value; e.target.style.width = "auto"; e.target.style.width = e.target.scrollWidth + "px"; e.target.style.height = "auto"; e.target.style.height = e.target.scrollHeight + "px"; }}
          @keydown=${(e) => {
            if (e.key === "Escape") { this._text = null; this._finishText(); }
            else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) this._commitText();
            e.stopPropagation();
          }}
          @blur=${() => this._commitText()}></textarea>` : nothing}
      </div>
    `;
  }
}
customElements.define("dx-canvas", DxCanvas);

// ───────── 레이어 패널 (좌측) ─────────
class DxLayerPanel extends LitElement {
  static properties = { app: { attribute: false }, _v: { state: true }, _menu: { state: true }, _editing: { state: true } };
  static styles = [controls, css`
    :host {
      grid-area: layers; display: flex; flex-direction: column; width: 240px;
      background: var(--bg-panel); border-right: 1px solid var(--line); overflow: hidden;
    }
    .head {
      padding: 12px 12px 8px; font-size: 11px; font-weight: 600; color: var(--fg);
      display: flex; align-items: center; justify-content: space-between;
    }
    .head .add { width: 24px; height: 24px; padding: 0; justify-content: center; }
    .list { flex: 1; overflow-y: auto; padding: 0 6px 8px; position: relative; }
    .menu {
      position: absolute; right: 10px; top: 2px; z-index: 40; background: var(--bg-panel);
      border: 1px solid var(--line); border-radius: 9px; padding: 5px; min-width: 160px;
      box-shadow: var(--shadow-menu); display: flex; flex-direction: column; gap: 1px;
    }
    .menu button { width: 100%; justify-content: flex-start; height: 30px; color: var(--fg); }
    .menu button:hover { background: var(--accent); color: #fff; }
    .row {
      display: flex; align-items: center; gap: 7px; padding: 0 6px; height: 32px;
      border-radius: var(--radius); font-size: 11.5px; cursor: default; color: var(--fg-2);
    }
    .row:hover { background: var(--bg-hover); color: var(--fg); }
    .row.sel { background: var(--accent-soft); color: var(--fg); }
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
  constructor() { super(); this._v = 0; this._menu = false; this._editing = null; }
  connectedCallback() {
    super.connectedCallback();
    this._onChange = () => { this._v++; };
    this.app?.addEventListener("changed", this._onChange);
    this._onDoc = (e) => { if (this._menu && !e.composedPath().includes(this)) this._menu = false; };
    document.addEventListener("click", this._onDoc);
  }
  disconnectedCallback() {
    this.app?.removeEventListener("changed", this._onChange);
    document.removeEventListener("click", this._onDoc);
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
      this.app.apply([B.addPaintLayer(file.name.replace(/\.[^.]+$/, ""), B.pngBase64(b64))]);
    };
    reader.readAsDataURL(file);
    e.target.value = "";
  }
  _rename(id, v) {
    this._editing = null;
    const name = v.trim();
    if (name) this.app.apply([B.setProps(id, { name })]);
  }
  render() {
    const layers = this.app ? this.app.layers() : [];
    const selIds = this.app?.selectedIds ?? [];
    return html`
      <div class="head"><span>레이어</span>
        <button class="add" title="레이어 추가" @click=${(e) => { e.stopPropagation(); this._menu = !this._menu; }}>${icon("plus")}</button>
      </div>
      <div class="list">
        ${this._menu ? html`
          <div class="menu">
            <button @click=${() => this._addLayer(null)}>${icon("square")}빈 레이어</button>
            <button @click=${() => this._addLayer([255, 255, 255, 255])}>${icon("squareFill")}단색 레이어</button>
            <button @click=${() => this.renderRoot.querySelector("#png2").click()}>${icon("image")}이미지 가져오기</button>
            <input id="png2" type="file" accept="image/png" style="display:none" @change=${(e) => this._addPng(e)} />
          </div>` : nothing}
        ${layers.length === 0 ? html`<div class="empty">레이어가 없습니다.<br>도형을 그리거나 dx CLI로 추가하세요.</div>` : nothing}
        ${layers.map((l) => html`
          <div class="row ${selIds.includes(l.id) ? "sel" : ""}"
            @click=${(e) => (e.shiftKey ? this.app.toggleSelect(l.id) : this.app.select(l.id))}>
            <span class="ord">
              <button title="위로" @click=${(e) => { e.stopPropagation(); this.app.raise(l.id); }}>${icon("chevUpS", 9)}</button>
              <button title="아래로" @click=${(e) => { e.stopPropagation(); this.app.lower(l.id); }}>${icon("chevDownS", 9)}</button>
            </span>
            <span class="tic">${icon("square", 11)}</span>
            <span class="name" @dblclick=${(e) => { e.stopPropagation(); this._editing = l.id; }}>
              ${this._editing === l.id
                ? html`<input .value=${l.name} autofocus
                    @click=${(e) => e.stopPropagation()}
                    @keydown=${(e) => { if (e.key === "Enter") this._rename(l.id, e.target.value); if (e.key === "Escape") this._editing = null; e.stopPropagation(); }}
                    @blur=${(e) => this._rename(l.id, e.target.value)} />`
                : l.name}
            </span>
            <button class="b" title="표시/숨김"
              @click=${(e) => { e.stopPropagation(); this.app.apply([B.setProps(l.id, { visible: !l.visible })]); }}>
              ${icon(l.visible ? "eye" : "eyeOff", 13)}</button>
            <button class="b danger" title="삭제"
              @click=${(e) => { e.stopPropagation(); this.app.apply([B.deleteLayer(l.id)]); }}>${icon("trash", 13)}</button>
          </div>`)}
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
      grid-area: props; display: block; width: 240px; background: var(--bg-panel);
      border-left: 1px solid var(--line); overflow-y: auto;
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
    if (!l) return html`<div class="head">Design</div>
      <div class="empty">선택된 레이어가 없습니다.<br>V 도구로 캔버스에서 선택하세요.</div>`;
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
      grid-template-rows: 44px 1fr;
      grid-template-columns: auto 1fr auto;
      grid-template-areas: "topbar topbar topbar" "layers canvas props";
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
    if (meta && k === "d") { e.preventDefault(); this.app.duplicateMany(ids); return; }
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
      const map = { v: "select", r: "rect", e: "ellipse", l: "line", t: "text", i: "eyedrop" };
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
      <dx-layer-panel .app=${this.app}></dx-layer-panel>
      <dx-canvas .app=${this.app} .toolState=${this._tool}
        @zoom-changed=${(e) => { this._zoom = e.detail; }}
        @picked-color=${(e) => { this._topbar?.setColor(e.detail); this._topbar?.setTool("select"); }}
        @text-finished=${() => this._topbar?.setTool("select")}></dx-canvas>
      <dx-props .app=${this.app}></dx-props>
    `;
  }
}
customElements.define("app-shell", AppShell);
