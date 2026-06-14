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
  terminal: svg`<rect x="2" y="3" width="12" height="10" rx="1.5"/><path d="M4.5 6l2 2-2 2M8 10h3.5"/>`,
  close: svg`<path d="M4 4l8 8M12 4l-8 8"/>`,
  rot90: svg`<path d="M12.5 6.5a5 5 0 10.5 3"/><path d="M12.5 2.5v4h-4"/>`,
  flipH: svg`<path d="M8 2v12"/><path d="M5.5 5L2.5 8l3 3z" fill="currentColor" stroke="none"/><path d="M10.5 5l3 3-3 3"/>`,
  flipV: svg`<path d="M2 8h12"/><path d="M5 5.5L8 2.5l3 3z" fill="currentColor" stroke="none"/><path d="M5 10.5l3 3 3-3"/>`,
  lock: svg`<rect x="4" y="7" width="8" height="6" rx="1"/><path d="M5.5 7V5.5a2.5 2.5 0 015 0V7"/>`,
  unlock: svg`<rect x="4" y="7" width="8" height="6" rx="1"/><path d="M5.5 7V5.5a2.5 2.5 0 015-.8"/>`,
  minus: svg`<path d="M3.5 8h9"/>`,
  polygonFill: svg`<path d="M8 2.5L13.2 6.3 11.2 12.4H4.8L2.8 6.3Z" fill="currentColor" stroke="none"/>`,
  curve: svg`<path d="M2 12.5C5 12.5 5.5 3.5 8.5 3.5c2.8 0 2.3 6 5.5 6"/>`,
};
const icon = (name, size = 15) => svg`
  <svg viewBox="0 0 16 16" width=${size} height=${size} fill="none"
    stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"
    style="display:block">${P[name]}</svg>`;

const SHAPES = [
  { id: "rect", ic: "squareFill", label: "사각형", key: "R" },
  { id: "ellipse", ic: "circleFill", label: "타원", key: "E" },
  { id: "polygon", ic: "polygonFill", label: "다각형", key: "P" },
  { id: "line", ic: "line", label: "선", key: "L" },
  { id: "curve", ic: "curve", label: "곡선", key: "C" },
];
const isShapeTool = (t) => SHAPES.some((s) => s.id === t);
const needsWidth = (t) => t === "line" || t === "brush" || t === "curve";
const polygonPts = B.polygonPoints; // 엔진 regular_polygon_points 미러(bridge 공용).

/** uniform Catmull-Rom 평탄화(끝점 클램프) — 엔진 catmull_rom_flatten의 미리보기 미러. */
const curveFlatten = (pts) => {
  const n = pts.length / 2;
  if (n < 2) return [...pts];
  const P = (i) => {
    const j = Math.max(0, Math.min(n - 1, i));
    return [pts[2 * j], pts[2 * j + 1]];
  };
  const out = [pts[0], pts[1]];
  for (let i = 0; i < n - 1; i++) {
    const [p0, p1, p2, p3] = [P(i - 1), P(i), P(i + 1), P(i + 2)];
    const m1 = [(p2[0] - p0[0]) * 0.5, (p2[1] - p0[1]) * 0.5];
    const m2 = [(p3[0] - p1[0]) * 0.5, (p3[1] - p1[1]) * 0.5];
    const chord = Math.hypot(p2[0] - p1[0], p2[1] - p1[1]);
    const steps = Math.max(4, Math.min(48, Math.ceil(chord / 2.5)));
    for (let k = 1; k <= steps; k++) {
      const t = k / steps, t2 = t * t, t3 = t2 * t;
      const h00 = 2 * t3 - 3 * t2 + 1, h10 = t3 - 2 * t2 + t;
      const h01 = -2 * t3 + 3 * t2, h11 = t3 - t2;
      out.push(
        h00 * p1[0] + h10 * m1[0] + h01 * p2[0] + h11 * m2[0],
        h00 * p1[1] + h10 * m1[1] + h01 * p2[1] + h11 * m2[1],
      );
    }
  }
  return out;
};
const MAX_VIEWPORT_SIDE = 4096;
const MAX_VIEWPORT_PIXELS = 12_000_000;

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
    sides: { state: true },
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
    this.width = 4; this.radius = 12; this.sides = 5;
    this.fontSize = 32; this.fontName = "Pretendard"; this._fonts = null;
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
    if (t === "text") this._loadFonts();
    this.tool = t;
    if (isShapeTool(t)) this._shape = t;
    this._menu = false;
    this._emit();
  }
  _toolState() {
    return {
      tool: this.tool, rgba: RGBA(this.color, this.alpha),
      width: this.width, radius: this.radius, sides: this.sides, size: this.fontSize,
      font: this.fontName === "Pretendard" ? null : this.fontName,
    };
  }

  _loadFonts() {
    if (this._fonts || this._fontsLoading) return;
    this._fontsLoading = true;
    (async () => {
      this._fonts = await this.app?.fontList?.() ?? ["Pretendard"];
      this._fontsLoading = false;
      this.requestUpdate();
    })();
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
        ${t("brush", "pencil", "브러시 (B)")}
        ${t("frame", "frame", "프레임 (F)")}
        ${t("text", "text", "텍스트 (T)")}
        <button title="이미지(PNG) 레이어" @click=${() => this.renderRoot.querySelector("#png").click()}>${icon("image")}</button>
        <input id="png" type="file" accept="image/png" style="display:none" @change=${(e) => this._addPng(e)} />
        ${this._menu ? html`
          <div class="menu">
            ${SHAPES.map((s) => html`
              <button title=${s.id === "curve" ? "클릭으로 점 추가, 더블클릭/Enter로 완성" : s.label}
                @click=${() => this._pick(s.id)}>${icon(s.ic)}${s.label}
                ${s.key ? html`<span class="key">${s.key}</span>` : nothing}</button>`)}
          </div>` : nothing}
      </div>
      ${isDraw ? html`
        <div class="opts">
          <span class="swatch" style="background:${this.color}">
            <input type="color" .value=${this.color} @input=${(e) => { this.color = e.target.value; this._emit(); }} /></span>
          <button title="스포이드" @click=${() => this._pick("eyedrop")}>${icon("dropper", 13)}</button>
          <label>A<input type="range" min="0" max="1" step="0.05" .value=${String(this.alpha)}
            @input=${(e) => { this.alpha = +e.target.value; this._emit(); }} /></label>
          ${needsWidth(this.tool) ? html`<label>W<input class="num" type="number" min="1" max="100" .value=${String(this.width)}
            @change=${(e) => { this.width = +e.target.value || 1; this._emit(); }} /></label>` : nothing}
          ${this.tool === "polygon" ? html`<label>변<input class="num" type="number" min="3" max="64" .value=${String(this.sides)}
            @change=${(e) => { this.sides = Math.max(3, Math.min(64, Math.round(+e.target.value) || 5)); this._emit(); }} /></label>` : nothing}
          ${this.tool === "text" ? html`
            <label>크기<input class="num" type="number" min="6" max="400" .value=${String(this.fontSize)}
              @change=${(e) => { this.fontSize = +e.target.value || 12; this._emit(); }} /></label>
            <select style="max-width:150px" .value=${this.fontName} title="글꼴"
              @change=${async (e) => {
                const v = e.target.value;
                if (await this.app?.ensureFont?.(v) !== false) { this.fontName = v; this._emit(); }
              }}>
              ${(this._fonts ?? ["Pretendard"]).map((f) => html`<option value=${f} ?selected=${f === this.fontName}>${f}</option>`)}
            </select>` : nothing}
        </div>` : nothing}
      <div class="corner">
        <button class="ico" title="undo (Cmd+Z)" ?disabled=${!this.app?.canUndo()} @click=${() => this.app.undo()}>${icon("undo")}</button>
        <button class="ico" title="redo (Cmd+Shift+Z)" ?disabled=${!this.app?.canRedo()} @click=${() => this.app.redo()}>${icon("redo")}</button>
        <span class="sep"></span>
        <div class="zoom">
          <button class="ico" title="축소" @click=${() => this._zoomCmd("out")}>−</button>
          <span class="pct" title="100%: 디바이스 픽셀 1:1 (Shift+0) / 맞춤 (Shift+1)"
            @click=${() => this._zoomCmd("reset")}>${Math.round(this.zoom * (window.devicePixelRatio || 1) * 100)}%</span>
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
          <button @click=${() => { this._exportMenu = false; this.dispatchEvent(new CustomEvent("save-psd", { bubbles: true, composed: true })); }}>
            ${icon("export", 13)}PSD 내보내기
          </button>
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
    #base {
      will-change: transform;
      contain: layout paint size;
    }
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
    this._hoverFrameId = null;
    this._space = false; this._text = null;
    this._editPts = null; // 점 편집 모드: { id } — 곡선/선/다각형 더블클릭으로 진입.
    this._bgMode = localStorage.getItem("dx.canvas.bg") || "dot";
  }
  connectedCallback() {
    super.connectedCallback();
    this._onChange = (e) => {
      this._v++;
      if (e?.detail?.geometryOnly) {
        clearTimeout(this._pendingMoveTimer);
        this._pendingMoveTimer = setTimeout(() => {
          this._pendingMove = null;
          this._drawOverlay();
        }, 140);
      } else {
        this._pendingMove = null;
      }
      if (e?.detail?.layoutChanged) {
        clearTimeout(this._viewportTimer);
        this._viewportTimer = 0;
        // force 아님: 이미 뷰포트가 덮고 있으면 캔버스 재할당·재합성 강제하지 않는다
        // (편집마다 강제하면 12MP 백킹 재할당 + 전체 재합성으로 스터터).
        this._applyZoom();
      }
      // 오버레이는 Lit updated()에서 한 번 그린다(여기서 또 그리면 편집당 2회 풀 리페인트).
    };
    this.app?.addEventListener("changed", this._onChange);
    this._mv = (e) => this._move(e); this._up = (e) => this._end(e);
    window.addEventListener("pointermove", this._mv);
    window.addEventListener("pointerup", this._up);
    this._kd = (e) => {
      if (this._isTyping(e)) return;
      if (this._drag?.mode === "curve") {
        if (e.key === "Enter") { e.preventDefault(); this._commitCurve(); return; }
        if (e.key === "Escape") { e.preventDefault(); this._cancelCurve(); return; }
      }
      // 점 드래그 중 Escape = 그 드래그만 취소(커밋 안 함).
      if (this._drag?.mode === "point" && e.key === "Escape") {
        e.preventDefault();
        this._drag = null;
        this.app.renderer.excludeId = null;
        this.app.renderer.markDirty();
        this._drawOverlay();
        return;
      }
      if (this._editPts && this._drag?.mode !== "point" && (e.key === "Enter" || e.key === "Escape")) {
        e.preventDefault();
        this._exitPointEdit();
        return;
      }
      if (e.code === "Space") { this._space = true; this.style.cursor = "grab"; e.preventDefault(); }
    };
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
    clearTimeout(this._pendingMoveTimer);
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
    this._applyZoom(true);
    this.addEventListener("pointerdown", (e) => this._down(e));
    this.base.addEventListener("dblclick", (e) => this._dblclick(e));
    this.addEventListener("contextmenu", (e) => this._context(e));
    this.addEventListener("pointerleave", () => {
      if (this._hoverId != null || this._hoverFrameId != null) {
        this._hoverId = null;
        this._hoverFrameId = null;
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
    this._updateBgGrid();
  }

  /** 도트 격자를 월드 좌표에 고정 — 팬/줌을 따라 움직인다(간격 = 24 doc px). */
  _updateBgGrid() {
    if (this._bgMode !== "dot") {
      this.style.backgroundImage = "";
      this.style.backgroundSize = "";
      this.style.backgroundPosition = "";
      return;
    }
    const z = this._zoom;
    const step = 24 * z;
    if (step < 7) {
      // 줌아웃에서 도트가 노이즈가 되는 밀도 — 격자 숨김.
      this.style.backgroundImage = "none";
      return;
    }
    const dpr = Math.max(1, window.devicePixelRatio || 1);
    const snap = (v) => Math.round(v * dpr) / dpr;
    this.style.backgroundImage = ""; // 스타일시트의 radial-gradient로 복귀.
    this.style.backgroundSize = `${step}px ${step}px`;
    // 월드 (0,0)의 화면 위치에 격자 원점을 고정.
    this.style.backgroundPosition = `${snap(-this._origin.x * z)}px ${snap(-this._origin.y * z)}px`;
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
    this._drawOverlay(); // 기존 편집이면 셀렉션 크롬 유지, 새 입력만 숨김(_drawOverlay 참조).
    this.updateComplete.then(() => {
      const ta = this.renderRoot.querySelector("textarea.txt");
      if (!ta) return;
      // 열리자마자 내용 크기에 맞춤 — 40px 미니박스로 보이는 문제(이상한 핸들로 오인) 방지.
      ta.style.width = "auto"; ta.style.width = ta.scrollWidth + "px";
      ta.style.height = "auto"; ta.style.height = ta.scrollHeight + "px";
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

  /** select 도구 더블클릭: 텍스트 = 인라인 편집, 곡선/선/다각형 = 점 편집 진입.
   *  곡선 그리기 도구 중에는 커밋. */
  _dblclick(e) {
    try {
      const tool = this.toolState?.tool ?? "select";
      if (tool === "curve") {
        e.preventDefault();
        this._commitCurve();
        return;
      }
      if (tool !== "select") return;
      const p = this._coords(e);
      const hit = this.app.hitTest(p.x, p.y);
      if (hit == null) {
        this._exitPointEdit();
        return;
      }
      const layer = this.app.layers().find((l) => l.id === hit);
      const meta = this.app.metaOf(layer);
      if (meta?.type !== "text") {
        e.preventDefault();
        this._enterPointEdit(hit, layer, meta);
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
        surface_size: layer.surface_size,
      };
      this._openText({
        x: meta.x, y: meta.y, value: meta.text,
        size: meta.size, rgba: meta.rgba,
        origText: meta.text, origSize: meta.size, bg: meta.bg ?? null, font: meta.font ?? null,
        editId: hit, xf, box: this.app.textBoxBounds(layer, meta),
      });
    } catch (err) {
      console.error("[text] 편집 진입 실패:", err);
    }
  }
  updated() {
    // 곡선 작성 중 도구가 바뀌면(단축키 등) 진행분 폐기 — 유령 곡선 잔류 방지.
    if (this._drag?.mode === "curve" && (this.toolState?.tool ?? "select") !== "curve") this._cancelCurve();
    // 점 편집 중 도구 변경·선택 해제·다른 레이어 선택이면 모드 종료.
    if (this._editPts) {
      const sel = this.app.selectedIds ?? [];
      const stale = (this.toolState?.tool ?? "select") !== "select"
        || sel.length !== 1 || sel[0] !== this._editPts.id
        || !this.app.layers().some((l) => l.id === this._editPts.id);
      if (stale) this._exitPointEdit();
    }
    this._applyZoom();
    this._drawOverlay();
  }

  _scheduleOverlay() {
    if (this._overlayRaf) return;
    this._overlayRaf = requestAnimationFrame(() => {
      this._overlayRaf = 0;
      this._drawOverlay();
    });
  }

  _scheduleViewportRefresh(delay = 90) {
    clearTimeout(this._viewportTimer);
    this._viewportTimer = setTimeout(() => {
      this._viewportTimer = 0;
      this._applyZoom(false, true);
      this._drawOverlay();
    }, delay);
  }

  _setStylePx(el, prop, value) {
    const next = `${value}px`;
    if (el.style[prop] !== next) el.style[prop] = next;
  }

  // ---- 줌/팬 ----
  get zoom() { return this._zoom; }
  /** 팬 전용 즉시 뷰 갱신 — 보존 프레임의 스크롤 경로(행 memmove + 스트립 재합성)가
   *  프레임당 수 ms라, 90ms 디바운스 없이 매 rAF 재합성해도 된다(패드 밖 빈 화면 제거).
   *  줌이 아직 뷰에 반영 안 된 상태(제스처 직후)면 false — 종전 디바운스 경로로. */
  _panView() {
    const r = this.app.renderer;
    if (!r.hasViewComposite?.() || !r.view) return false;
    if (typeof this.app.editor.render_frame !== "function") return false;
    const v = r.view;
    if (v.zoom !== this._zoom) return false;
    const padCss = 128;
    const z = this._zoom;
    r.setView(this._origin.x - padCss / z, this._origin.y - padCss / z, z, v.cssW, v.cssH, v.renderScale);
    return true;
  }
  _applyZoom(force = false, render = true) {
    if (!this.base) return;
    this._updateBgGrid();
    const z = this._zoom;
    const cw = Math.max(1, this.clientWidth);
    const ch = Math.max(1, this.clientHeight);
    if (this.app.renderer.hasViewComposite?.()) {
      // ── 화면 공간 렌더(Figma식) ──
      // 보이는 영역(+팬 여유 패드)만, 가능하면 디바이스 해상도로 직접 합성한다.
      // 합성 비용이 장면 크기와 무관해지고, 줌아웃에서 화면이 잘리는 상한도 없다.
      const dpr = Math.max(1, window.devicePixelRatio || 1);
      const padCss = 128;
      const bw = cw + padCss * 2;
      const bh = ch + padCss * 2;
      // 디바이스 픽셀 예산 내 최대 해상도. 초과 시 1.0으로 "추락"시키지 않고 연속 강등 —
      // 큰 창(레티나)에서 rs=1이 되면 화면 전체가 2x 업스케일로 흐려진다.
      const budget = 12_000_000;
      const rs = Math.min(dpr, Math.max(1, Math.sqrt(budget / (bw * bh))));
      const v = this.app.renderer.view;
      const covered = !force && v && v.zoom === z && v.renderScale === rs
        && v.x <= this._origin.x && v.y <= this._origin.y
        && v.x + v.cssW / v.zoom >= this._origin.x + cw / z
        && v.y + v.cssH / v.zoom >= this._origin.y + ch / z;
      if (!covered && render) {
        this.app.renderer.setView(this._origin.x - padCss / z, this._origin.y - padCss / z, z, bw, bh, rs);
      }
      const cur = this.app.renderer.view;
      if (cur) {
        // 줌 제스처 중(리컴포지트 전)에는 기존 버퍼를 CSS로 늘려/줄여 보여준다.
        this._setStylePx(this.base, "width", cur.cssW * (z / cur.zoom));
        this._setStylePx(this.base, "height", cur.cssH * (z / cur.zoom));
        // translate를 디바이스 픽셀 그리드에 스냅 — 분수 위치는 브라우저 리샘플로
        // 전체가 미세하게 블러된다(버퍼=디바이스 해상도인 의미가 사라짐).
        const snap = (v) => Math.round(v * dpr) / dpr;
        this.base.style.transform = `translate3d(${snap((cur.x - this._origin.x) * z)}px, ${snap((cur.y - this._origin.y) * z)}px, 0)`;
      }
      this.base.style.imageRendering = "auto"; // 버퍼≈디바이스 해상도 — CSS 스케일 거의 없음.
      this._sizeOverlay(cw, ch);
      return;
    }
    // ── 폴백: 종전 문서 해상도 영역 렌더(구버전 wasm 캐시) ──
    const visible = {
      x0: Math.floor(this._origin.x),
      y0: Math.floor(this._origin.y),
      x1: Math.ceil(this._origin.x + cw / z),
      y1: Math.ceil(this._origin.y + ch / z),
    };
    const pad = Math.min(2048, Math.max(64, Math.ceil(Math.max(cw, ch) / z / 4)));
    const target = this._viewportTarget(visible, pad);
    const vp = this.app.renderer.viewport;
    const covered = !force && vp
      && vp.x <= target.x && vp.y <= target.y
      && vp.x + vp.w >= target.x + target.w && vp.y + vp.h >= target.y + target.h;
    if (!covered && render) {
      this.app.renderer.setViewport(target.x, target.y, target.w, target.h);
    }
    const rv = this.app.renderer.viewport;
    this._setStylePx(this.base, "width", rv.w * z);
    this._setStylePx(this.base, "height", rv.h * z);
    this.base.style.transform = `translate3d(${(rv.x - this._origin.x) * z}px, ${(rv.y - this._origin.y) * z}px, 0)`;
    const ddpr = Math.max(1, window.devicePixelRatio || 1);
    this.base.style.imageRendering = z * ddpr >= 3 ? "pixelated" : "auto";
    this._sizeOverlay(cw, ch);
  }

  /** 오버레이 캔버스를 호스트 크기 × dpr로 유지(셀렉션 크롬은 항상 디바이스 해상도). */
  _sizeOverlay(ow, oh) {
    const dpr = Math.max(1, window.devicePixelRatio || 1);
    const bw = Math.max(1, Math.round(ow * dpr));
    const bh = Math.max(1, Math.round(oh * dpr));
    if (this.overlay.width !== bw) this.overlay.width = bw;
    if (this.overlay.height !== bh) this.overlay.height = bh;
    this._overlayDpr = dpr;
    this._overlayW = ow;
    this._overlayH = oh;
    this._setStylePx(this.overlay, "width", ow);
    this._setStylePx(this.overlay, "height", oh);
  }

  _overlayCtx(clear = false) {
    const o = this.overlay.getContext("2d");
    const dpr = this._overlayDpr || Math.max(1, window.devicePixelRatio || 1);
    o.setTransform(dpr, 0, 0, dpr, 0, 0);
    o.imageSmoothingEnabled = false;
    if (clear) o.clearRect(0, 0, this._overlayW || this.clientWidth, this._overlayH || this.clientHeight);
    return o;
  }
  _setZoom(z, cx, cy, immediate = false) {
    const old = this._zoom;
    z = Math.min(8, Math.max(0.05, z));
    if (z === old) return;
    const rect = this.getBoundingClientRect();
    const px = (cx ?? rect.left + rect.width / 2) - rect.left;
    const py = (cy ?? rect.top + rect.height / 2) - rect.top;
    const world = { x: this._origin.x + px / old, y: this._origin.y + py / old };
    this._zoom = z;
    this._origin = { x: world.x - px / z, y: world.y - py / z };
    if (immediate) {
      clearTimeout(this._viewportTimer);
      this._viewportTimer = 0;
      this._applyZoom(true);
      this._drawOverlay();
    } else {
      this._applyZoom(false, false);
      this._scheduleViewportRefresh(70);
      this._scheduleOverlay();
    }
    this.dispatchEvent(new CustomEvent("zoom-changed", { detail: z, bubbles: true, composed: true }));
  }
  zoomCmd(action) {
    if (action === "in") this._setZoom(this._zoom * 1.25);
    else if (action === "out") this._setZoom(this._zoom / 1.25);
    // 100% = 문서 1px : 디바이스 1px (Photoshop 방식). 레티나에서 CSS 1:1(=디바이스 2x)은
    // 래스터 업스케일이라 텍스트가 흐릿하다 — 진짜 1:1 지점으로 리셋.
    else if (action === "reset") this._setZoom(1 / Math.max(1, window.devicePixelRatio || 1), undefined, undefined, true);
    else if (action === "selection") {
      // 선택(레이어들/프레임)에 맞춰 줌 — Figma Shift+2.
      const boxes = [];
      const f = this.app.getSelectedFrame?.();
      if (f) boxes.push({ x: f.x, y: f.y, w: f.w, h: f.h });
      for (const l of this.app.selectedLayers?.() ?? []) {
        const b = this.app.displayAABB(l);
        if (b) boxes.push(b);
      }
      if (!boxes.length) return;
      let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
      for (const b of boxes) {
        x0 = Math.min(x0, b.x); y0 = Math.min(y0, b.y);
        x1 = Math.max(x1, b.x + b.w); y1 = Math.max(y1, b.y + b.h);
      }
      const margin = 80;
      const z = Math.min((this.clientWidth - margin * 2) / (x1 - x0 || 1), (this.clientHeight - margin * 2) / (y1 - y0 || 1));
      const zc = Math.min(8, Math.max(0.05, z));
      this._origin = {
        x: (x0 + x1) / 2 - this.clientWidth / 2 / zc,
        y: (y0 + y1) / 2 - this.clientHeight / 2 / zc,
      };
      const old = this._zoom;
      this._zoom = zc;
      clearTimeout(this._viewportTimer);
      this._viewportTimer = 0;
      this._applyZoom(true);
      this._drawOverlay();
      if (old !== zc) this.dispatchEvent(new CustomEvent("zoom-changed", { detail: zc, bubbles: true, composed: true }));
    }
    else if (action === "fit") {
      const W = this.app.editor.width(), H = this.app.editor.height();
      const z = Math.min((this.clientWidth - 96) / W, (this.clientHeight - 96) / H);
      this._origin = { x: -48 / Math.max(z, 0.05), y: -48 / Math.max(z, 0.05) };
      this._setZoom(z, undefined, undefined, true);
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
    if (this._panView()) {
      this._applyZoom(false, false);
    } else {
      this._applyZoom(false, false);
      this._scheduleViewportRefresh();
    }
    this._scheduleOverlay();
  }

  // ---- 좌표 ----
  _coords(e) {
    const r = this.getBoundingClientRect();
    return {
      x: this._origin.x + (e.clientX - r.left) / this._zoom,
      y: this._origin.y + (e.clientY - r.top) / this._zoom,
    };
  }

  _dragDistanceFromStart(d, p) {
    return Math.hypot(p.x - d.start.x, p.y - d.start.y);
  }

  _intersectRect(a, b) {
    const x0 = Math.max(a.x, b.x);
    const y0 = Math.max(a.y, b.y);
    const x1 = Math.min(a.x + a.w, b.x + b.w);
    const y1 = Math.min(a.y + a.h, b.y + b.h);
    if (x1 <= x0 || y1 <= y0) return null;
    return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
  }

  _viewportTarget(visible, pad) {
    const desired = {
      x: visible.x0 - pad,
      y: visible.y0 - pad,
      w: visible.x1 - visible.x0 + pad * 2,
      h: visible.y1 - visible.y0 + pad * 2,
    };
    const scene = this.app.sceneBounds?.();
    const bounded = scene ? (this._intersectRect(desired, scene) ?? scene) : desired;
    return this._capViewportTarget(bounded, visible);
  }

  _capViewportTarget(target, visible) {
    let { x, y, w, h } = target;
    const cx = (visible.x0 + visible.x1) / 2;
    const cy = (visible.y0 + visible.y1) / 2;
    let maxW = MAX_VIEWPORT_SIDE;
    let maxH = MAX_VIEWPORT_SIDE;
    if (maxW * maxH > MAX_VIEWPORT_PIXELS) {
      const s = Math.sqrt(MAX_VIEWPORT_PIXELS / (maxW * maxH));
      maxW = Math.floor(maxW * s);
      maxH = Math.floor(maxH * s);
    }
    if (w > maxW) {
      const nx = Math.max(x, Math.min(x + w - maxW, Math.floor(cx - maxW / 2)));
      x = nx;
      w = maxW;
    }
    if (h > maxH) {
      const ny = Math.max(y, Math.min(y + h - maxH, Math.floor(cy - maxH / 2)));
      y = ny;
      h = maxH;
    }
    return { x, y, w: Math.max(1, w), h: Math.max(1, h) };
  }

  _screen(p) {
    return { x: (p.x - this._origin.x) * this._zoom, y: (p.y - this._origin.y) * this._zoom };
  }

  _frameLabelHit(f, e) {
    if (!e) return false;
    const host = this.getBoundingClientRect();
    const sx = e.clientX - host.left;
    const sy = e.clientY - host.top;
    const fp = this._screen({ x: f.x, y: f.y });
    const labelW = Math.max(70, String(f.name ?? "").length * 7 + 12);
    return sx >= fp.x - 3 && sx <= fp.x + labelW && sy >= fp.y - 22 && sy <= fp.y + 4;
  }

  _frameAt(p, e = null) {
    const tol = 6 / (this._zoom || 1);
    for (const f of [...(this.app.frames?.() ?? [])].reverse()) {
      if (this._frameLabelHit(f, e)) return f;
      const inside = p.x >= f.x - tol && p.x <= f.x + f.w + tol && p.y >= f.y - tol && p.y <= f.y + f.h + tol;
      if (!inside) continue;
      return f;
    }
    return null;
  }

  _frameGeom(f) {
    if (!f) return null;
    const c4 = [
      this._screen({ x: f.x, y: f.y }),
      this._screen({ x: f.x + f.w, y: f.y }),
      this._screen({ x: f.x + f.w, y: f.y + f.h }),
      this._screen({ x: f.x, y: f.y + f.h }),
    ];
    const mid = (a, b) => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
    return {
      frame: f,
      c4,
      handles: [
        { k: "tl", p: c4[0] }, { k: "tm", p: mid(c4[0], c4[1]) },
        { k: "tr", p: c4[1] }, { k: "mr", p: mid(c4[1], c4[2]) },
        { k: "br", p: c4[2] }, { k: "bm", p: mid(c4[2], c4[3]) },
        { k: "bl", p: c4[3] }, { k: "ml", p: mid(c4[3], c4[0]) },
      ],
    };
  }

  _hitFrameHandle(e) {
    const f = this.app.getSelectedFrame?.();
    if (!f) return null;
    const g = this._frameGeom(f);
    const r = this.getBoundingClientRect();
    const sx = e.clientX - r.left, sy = e.clientY - r.top;
    for (const h of g.handles) {
      if (Math.hypot(h.p.x - sx, h.p.y - sy) <= HANDLE) return { h, g };
    }
    return null;
  }

  _context(e) {
    e.preventDefault();
    const p = this._coords(e);
    const host = this.getBoundingClientRect();
    const frame = this._frameAt(p, e);
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
    const meta = this.app.metaOf(sel);
    const isText = meta?.type === "text";
    const tb = isText ? this.app.textBoxBounds(sel, meta) : null;
    if (tb) b = [tb.x, tb.y, tb.w, tb.h];
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
    return { sel, b, t, tp, c4, handles, rot, ctrS, z, isText };
  }
  /** 핸들용 지오메트리 — 정확히 1개 선택일 때만(다중 선택은 핸들 비활성). */
  _selGeom() {
    if ((this.app.selectedIds?.length ?? 0) !== 1) return null;
    const sel = this.app.getSelected?.();
    if (!sel) return null;
    return this._selGeomFor(sel);
  }

  _drawSelectionGeom(o, g, acc, single, dx = 0, dy = 0) {
    o.beginPath();
    o.moveTo(g.c4[0].x + dx, g.c4[0].y + dy);
    for (let i = 1; i < 4; i++) o.lineTo(g.c4[i].x + dx, g.c4[i].y + dy);
    o.closePath();
    o.fillStyle = "rgba(13,153,255,0.08)";
    o.fill();
    o.strokeStyle = acc; o.lineWidth = 2;
    o.stroke();
    if (!single) return;
    const tm = { x: (g.c4[0].x + g.c4[1].x) / 2 + dx, y: (g.c4[0].y + g.c4[1].y) / 2 + dy };
    o.beginPath(); o.moveTo(tm.x, tm.y); o.lineTo(g.rot.x + dx, g.rot.y + dy); o.stroke();
    o.beginPath(); o.arc(g.rot.x + dx, g.rot.y + dy, 4.5, 0, 7); o.fillStyle = "#fff"; o.fill(); o.stroke();
    if (g.isText) return;
    for (const h of g.handles) {
      const hx = Math.round(h.p.x + dx), hy = Math.round(h.p.y + dy);
      o.fillStyle = "#fff";
      o.fillRect(hx - HANDLE / 2, hy - HANDLE / 2, HANDLE, HANDLE);
      o.strokeStyle = acc; o.lineWidth = 1;
      o.strokeRect(hx - HANDLE / 2 + 0.5, hy - HANDLE / 2 + 0.5, HANDLE - 1, HANDLE - 1);
    }
  }

  _drawFrameHandles(o, g, acc) {
    for (const h of g.handles) {
      const hx = Math.round(h.p.x), hy = Math.round(h.p.y);
      o.fillStyle = "#fff";
      o.fillRect(hx - HANDLE / 2, hy - HANDLE / 2, HANDLE, HANDLE);
      o.strokeStyle = acc; o.lineWidth = 1;
      o.strokeRect(hx - HANDLE / 2 + 0.5, hy - HANDLE / 2 + 0.5, HANDLE - 1, HANDLE - 1);
    }
  }

  /** 프레임 리사이즈 결과 사각형 — 미리보기와 커밋이 같은 수식을 쓴다.
   *  최소 1px 클램프 시 잡은 핸들의 "반대쪽" 변이 고정돼야 한다(좌 핸들이면 우변 고정,
   *  우 핸들이면 좌변 고정 — 이전엔 우/하 핸들에서 프레임이 커서를 따라 미끄러졌다). */
  _frameResizeRect(d) {
    const b = d.base;
    const dx = Math.round((d.cur?.x ?? d.start.x) - d.start.x);
    const dy = Math.round((d.cur?.y ?? d.start.y) - d.start.y);
    let x = b.x, y = b.y, w = b.w, h = b.h;
    if (d.h.includes("l")) { x = b.x + dx; w = b.w - dx; }
    if (d.h.includes("r")) w = b.w + dx;
    if (d.h.startsWith("t")) { y = b.y + dy; h = b.h - dy; }
    if (d.h.startsWith("b")) h = b.h + dy;
    if (w < 1) { x = d.h.includes("l") ? b.x + b.w - 1 : b.x; w = 1; }
    if (h < 1) { y = d.h.startsWith("t") ? b.y + b.h - 1 : b.y; h = 1; }
    return { ...b, x, y, w, h };
  }

  _previewFrame() {
    const d = this._drag;
    if (d?.mode !== "frame-resize") return null;
    return this._frameResizeRect(d);
  }

  _hitHandle(e) {
    const g = this._selGeom();
    if (!g) return null;
    const r = this.getBoundingClientRect();
    const sx = e.clientX - r.left, sy = e.clientY - r.top;
    const near = (p, rad) => Math.hypot(p.x - sx, p.y - sy) <= rad;
    if (near(g.rot, HANDLE)) return { type: "rotate", g };
    if (g.isText) return null;
    for (const h of g.handles) if (near(h.p, HANDLE)) return { type: "resize", h, g };
    return null;
  }

  /** 점 편집 모드에서 앵커 핸들 히트 — 가장 가까운(반경 내) 앵커 인덱스. */
  _hitAnchor(e) {
    const g = this._pointGeom();
    if (!g) return null;
    const r = this.getBoundingClientRect();
    const sx = e.clientX - r.left, sy = e.clientY - r.top;
    let best = null, bd = HANDLE + 2;
    for (let i = 0; i < g.screen.length; i++) {
      const d = Math.hypot(g.screen[i].x - sx, g.screen[i].y - sy);
      if (d <= bd) { bd = d; best = i; }
    }
    return best == null ? null : { idx: best, g };
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
    if (e.button != null && e.button !== 0) return;
    const path = e.composedPath?.() ?? [];
    // 텍스트 편집 textarea 안 클릭은 브라우저 기본(캐럿/드래그 선택)에 맡긴다.
    if (path.some((el) => el?.classList?.contains?.("ctx") || el?.classList?.contains?.("menu") || el?.classList?.contains?.("txt"))) return;
    // 편집 중 바깥 클릭 = 커밋하고 클릭 소비(Figma 동작).
    // blur보다 pointerdown이 먼저라 여기서 커밋하지 않으면 같은 클릭이 새 입력박스를
    // 열어 "편집창이 클릭 위치로 이동"하는 버그가 된다.
    if (this._text) {
      this._commitText();
      return;
    }
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
    if (tool === "curve") {
      // 클릭마다 앵커 추가 — 더블클릭/Enter 커밋, Escape 취소(_end는 드래그를 유지).
      e.preventDefault();
      const d = this._drag;
      if (d?.mode === "curve") {
        const lx = d.pts[d.pts.length - 2], ly = d.pts[d.pts.length - 1];
        // 더블클릭의 1·2차 클릭이 같은 자리에 점을 중복 추가하지 않게 최소 간격.
        if (Math.hypot(p.x - lx, p.y - ly) >= 3 / (this._zoom || 1)) d.pts.push(p.x, p.y);
        d.cur = p;
      } else {
        this._drag = { mode: "curve", pts: [p.x, p.y], cur: p };
      }
      this._drawCurveGhost();
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
      // 점 편집 모드: 앵커 핸들 잡기 우선. 빈 곳 클릭은 모드 이탈(아래 일반 select로 폴스루).
      if (this._editPts) {
        const ph = this._hitAnchor(e);
        if (ph) {
          const g = ph.g;
          // 정다각형은 첫 드래그에서 자유 다각형(polygon_path)으로 굳혀 꼭짓점을 자유 편집.
          const pts = g.kind === "polygon" ? [...g.pts] : [...g.pts];
          // 원본 래스터는 드래그 동안 화면에서 제외(ghost가 대체) — 이중 표시 방지.
          this.app.renderer.excludeId = g.l.id;
          this.app.renderer.markDirty();
          this._drag = { mode: "point", id: g.l.id, idx: ph.idx, geom: g, pts, start: p, moved: false };
          return;
        }
        // 앵커 밖 클릭 = 점 편집 종료(이어서 일반 select 동작).
        this._exitPointEdit();
      }
      // 핸들 우선(선택 유지한 채 리사이즈/회전).
      const fh = this._hitFrameHandle(e);
      if (fh) {
        const f = fh.g.frame;
        this._drag = { mode: "frame-resize", id: f.id, h: fh.h.k, start: p, cur: p, base: { ...f } };
        return;
      }
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
        const frame = this._frameAt(p, e);
        if (frame) {
          this.app.selectFrame(frame.id);
          this._drawOverlay();
          return;
        }
        this.app.select(null);
        this.app.selectFrame(null);
        // 빈 공간 드래그 = 마퀴 다중선택(클릭이면 해제 — _end에서 판정).
        this._drag = { mode: "marquee", start: p, cur: p };
        return;
      }
      // 이미 선택된 레이어를 잡으면 선택 유지(다중 함께 이동), 아니면 단일 교체.
      if (!this.app.selectedIds.includes(hit)) this.app.select(hit);
      const all = this.app.layers();
      const bases = new Map();
      const overlayGeoms = [];
      let box = null; // 선택 레이어들의 합쳐진 AABB(스냅 기준 — 이동 중엔 dx/dy만 더하면 됨).
      for (const id of this.app.selectedIds) {
        const l = all.find((v) => v.id === id);
        if (!l) continue;
        bases.set(id, l.offset ?? [0, 0]);
        const geom = this._selGeomFor(l);
        if (geom) overlayGeoms.push(geom);
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
        box, snap: null, guides: null, overlayGeoms,
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
      if (this._panView()) {
        this._applyZoom(false, false); // translate만 갱신(스냅 잔차 ≤1px).
      } else {
        this._applyZoom(false, false);
        this._scheduleViewportRefresh();
      }
      this._scheduleOverlay();
      return;
    }
    const p = this._coords(e);
    if (d.mode === "move") {
      d.dx = Math.round(p.x - d.start.x);
      d.dy = Math.round(p.y - d.start.y);
      // ★스냅★ 합쳐진 AABB의 left/centerX/right(+세로)를 타깃과 비교, 화면 6px 이내면 보정.
      d.guides = null;
      if (!d.snap && this._dragDistanceFromStart(d, p) > 2 / (this._zoom || 1)) {
        d.snap = this._snapTargets(this.app.selectedIds);
      }
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
      this._scheduleOverlay();
    } else if (d.mode === "marquee") {
      d.cur = p;
      this._scheduleOverlay();
    } else if (d.mode === "rotate") {
      const a = Math.atan2(p.y - d.pv.y, p.x - d.pv.x);
      let deg = d.rot0 + ((a - d.a0) * 180) / Math.PI;
      if (e.shiftKey) deg = Math.round(deg / 15) * 15; // Shift = 15° 스냅
      d.provRot = Math.round(deg * 10) / 10;
      // 도형 중심(피벗)이 제자리에 머물도록 offset 보정.
      d.provOffset = this.app.computeAnchoredOffset(d.sel, null, d.provRot, d.pivot);
      this._scheduleOverlay();
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
      this._scheduleOverlay();
    } else if (d.mode === "brush") {
      // 점 간 최소 간격(줌 보정)으로 thinning — 과밀 점 방지.
      const minD = 1.5 / (this._zoom || 1);
      if (Math.hypot(p.x - d.last.x, p.y - d.last.y) >= minD) {
        d.pts.push(p.x, p.y);
        d.last = p;
        this._drawBrushGhost();
      }
    } else if (d.mode === "curve") {
      d.cur = p;
      this._drawCurveGhost();
    } else if (d.mode === "point") {
      // 잡은 앵커를 item 좌표로 환산해 갱신(Shift = 8방향 직교 스냅).
      d.moved = true;
      let wx = p.x, wy = p.y;
      if (e.shiftKey) {
        const base = d.geom.toWorld(d.geom.pts[2 * d.idx], d.geom.pts[2 * d.idx + 1]);
        const dx = wx - base.x, dy = wy - base.y;
        if (Math.abs(dx) > Math.abs(dy) * 2) wy = base.y;
        else if (Math.abs(dy) > Math.abs(dx) * 2) wx = base.x;
      }
      const [ix, iy] = d.geom.toItem(wx, wy);
      d.pts[2 * d.idx] = ix;
      d.pts[2 * d.idx + 1] = iy;
      this._drawPointGhost(d);
    } else if (d.mode === "frame") {
      d.cur = p;
      this._scheduleOverlay();
    } else if (d.mode === "frame-resize") {
      d.cur = p;
      this._scheduleOverlay();
    } else if (d.mode === "draw") {
      d.cur = p;
      this._drawGhost();
    }
  }

  _hover(e) {
    if ((this.toolState?.tool ?? "select") !== "select" || this._space || this._text) {
      if (this._hoverId != null || this._hoverFrameId != null) {
        this._hoverId = null;
        this._hoverFrameId = null;
        this._drawOverlay();
      }
      return;
    }
    const p = this._coords(e);
    const hit = this.app.hitTest(p.x, p.y);
    const next = hit == null || this.app.selectedIds?.includes(hit) ? null : hit;
    const frame = hit == null ? this._frameAt(p, e) : null;
    const nextFrame = frame?.id === this.app.selectedFrameId ? null : frame?.id ?? null;
    let dirty = false;
    if (next !== this._hoverId) {
      this._hoverId = next;
      dirty = true;
    }
    if (nextFrame !== this._hoverFrameId) {
      this._hoverFrameId = nextFrame;
      dirty = true;
    }
    if (dirty) this._drawOverlay();
  }
  _end() {
    const d = this._drag;
    if (!d) return;
    this.style.cursor = this._space ? "grab" : "";
    // 곡선은 클릭 누적 도구 — pointerup으로 끝나지 않는다(더블클릭/Enter가 커밋).
    if (d.mode === "curve") return;
    if (d.mode === "point") {
      this._drag = null;
      this.app.renderer.excludeId = null;
      if (d.moved) {
        // 변경된 앵커로 노드 보존 재래스터(정다각형은 polygon_path로 변환). 점 편집 유지.
        const ok = this.app.setShapePoints(d.id, d.pts);
        if (!ok) { this.app.renderer.markDirty(); }
      } else {
        this.app.renderer.markDirty();
      }
      this._drawOverlay();
      return;
    }
    if (d.mode === "pan") {
      this._drag = null;
      clearTimeout(this._viewportTimer);
      this._viewportTimer = 0;
      this._applyZoom(false, true);
      this._drawOverlay();
      return;
    }
    if (d.mode === "brush") {
      this._drag = null;
      const s = this.toolState;
      if (d.pts.length >= 2) {
        const item = B.path(d.pts, s.width, s.rgba);
        this.app.apply([
          B.addPaintLayer("brush", B.shapes([item]), { bind: "drawn" }),
          B.setProps("drawn", { meta: JSON.stringify({ type: "brush", item, rgba: s.rgba }) }),
        ]);
      }
      this._drawOverlay();
      return;
    }
    if (d.mode === "frame") {
      this._drag = null;
      const x = Math.min(d.start.x, d.cur.x), y = Math.min(d.start.y, d.cur.y);
      const w = Math.abs(d.cur.x - d.start.x), h = Math.abs(d.cur.y - d.start.y);
      if (w >= 8 && h >= 8) {
        const n = this.app.frames().length + 1;
        this.app.addFrame(`Frame ${n}`, x, y, w, h);
      }
      this._drawOverlay();
      return;
    }
    if (d.mode === "frame-resize") {
      this._drag = null;
      const { x, y, w, h } = this._frameResizeRect(d);
      const b = d.base;
      // 클릭만 하고 안 움직였으면 no-op SetFrames를 undo 히스토리에 남기지 않는다.
      if (x !== b.x || y !== b.y || w !== b.w || h !== b.h) {
        this.app.updateFrame(d.id, { x, y, w, h });
      } else {
        this._drawOverlay();
      }
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
        if (acts.length) {
          const res = this.app.apply(acts);
          if (res?.deferred || res?.optimistic || res?.geometryOnly) {
            this._pendingMove = d;
            this._drag = null;
            this._drawOverlay();
            return;
          }
        } else this._drawOverlay();
      } else this._drawOverlay();
      this._drag = null;
      return;
    }
    if (d.mode === "marquee") {
      this._drag = null;
      const x0 = Math.min(d.start.x, d.cur.x), y0 = Math.min(d.start.y, d.cur.y);
      const mw = Math.abs(d.cur.x - d.start.x), mh = Math.abs(d.cur.y - d.start.y);
      if (mw < 2 && mh < 2) {
        // 드래그 없는 빈 공간 클릭 = 선택 해제(기존 동작 유지).
        this.app.select(null);
        this.app.selectFrame(null);
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
      this._drag = null;
      if (d.provRot !== d.rot0)
        this.app.apply([B.setProps(d.id, { rotation: d.provRot, offset: d.provOffset ?? undefined })]);
      else this._drawOverlay();
      return;
    }
    if (d.mode === "resize") {
      this._drag = null;
      if (d.provScale[0] !== d.scale0[0] || d.provScale[1] !== d.scale0[1]) {
        // 도형/브러시는 scale 보간(저해상도·형태 붕괴) 대신 새 크기로 벡터 재래스터.
        if (!this.app.bakeShapeScale(d.id, d.provScale, d.provOffset ?? undefined))
          this.app.apply([B.setProps(d.id, { scale: d.provScale, offset: d.provOffset ?? undefined })]);
      } else this._drawOverlay();
      return;
    }
    this._drag = null;
    // draw 확정
    const o = this._overlayCtx(true);
    const { start, cur } = d;
    const s = this.toolState; const rgba = s.rgba;
    const bx = Math.min(start.x, cur.x), by = Math.min(start.y, cur.y);
    const bw = Math.abs(cur.x - start.x), bh = Math.abs(cur.y - start.y);
    const ecx = (start.x + cur.x) / 2, ecy = (start.y + cur.y) / 2;
    let shape, name, extra = {};
    switch (s.tool) {
      case "rect": if (bw < 1 || bh < 1) return; shape = B.rect(bx, by, bw, bh, rgba); name = "rect"; break;
      case "ellipse": if (bw < 1 || bh < 1) return; shape = B.ellipse(ecx, ecy, bw / 2, bh / 2, rgba); name = "ellipse"; break;
      case "polygon":
        if (bw < 1 || bh < 1) return;
        shape = B.polygon(ecx, ecy, bw / 2, bh / 2, s.sides ?? 5, rgba); name = "polygon";
        extra = { sides: s.sides ?? 5 };
        break;
      case "stroke-rect": if (bw < 1 || bh < 1) return; shape = B.strokeRect(bx, by, bw, bh, s.width, rgba); name = "stroke-rect"; break;
      case "stroke-ellipse": if (bw < 1 || bh < 1) return; shape = B.strokeEllipse(ecx, ecy, bw / 2, bh / 2, s.width, rgba); name = "stroke-ellipse"; break;
      case "rounded-rect": if (bw < 1 || bh < 1) return; shape = B.roundedRect(bx, by, bw, bh, s.radius, rgba); name = "rounded-rect"; break;
      case "line":
        if (Math.hypot(cur.x - start.x, cur.y - start.y) < 1) return;
        shape = B.line(start.x, start.y, cur.x, cur.y, s.width, rgba); name = "line"; break;
      default: return;
    }
    const res = this.app.apply([
      B.addPaintLayer(name, B.shapes([shape]), { bind: "drawn" }),
      B.setProps("drawn", { meta: JSON.stringify({ type: "shape", shape: s.tool, item: shape, fill: rgba, rgba, stroke: null, strokeWidth: 0, ...extra }) }),
    ]);
    this._finishDraw(res);
  }

  /** 곡선 커밋 — 앵커 2개 이상이면 레이어로 확정, 아니면 폐기. */
  _commitCurve() {
    const d = this._drag;
    if (d?.mode !== "curve") return;
    this._drag = null;
    const s = this.toolState;
    if (d.pts.length >= 4) {
      const rgba = s.rgba;
      const item = B.curve(d.pts, s.width, rgba);
      const res = this.app.apply([
        B.addPaintLayer("curve", B.shapes([item]), { bind: "drawn" }),
        B.setProps("drawn", { meta: JSON.stringify({ type: "shape", shape: "curve", item, fill: rgba, rgba, stroke: null, strokeWidth: 0 }) }),
      ]);
      this._finishDraw(res);
      return;
    }
    this._drawOverlay();
  }

  _cancelCurve() {
    this._drag = null;
    this._drawOverlay();
  }

  // ---- 점 편집 (곡선/선/다각형 핸들) ----
  /** 점 편집 진입 — 정다각형은 첫 꼭짓점 드래그에서 자유 다각형(polygon_path)으로 변환.
   *  비-identity 트랜스폼(회전·스케일≠1)은 재래스터 시 표면 중심 이동으로 위치가 틀어질
   *  수 있어 진입하지 않는다(도형 리사이즈는 bake로 scale 1 복귀 — 실사용 제약 적음). */
  _enterPointEdit(id, layer, meta) {
    if (!layer || !this.app.shapePoints?.(layer, meta)) return;
    const identity = (layer.rotation ?? 0) === 0
      && (layer.scale?.[0] ?? 1) === 1 && (layer.scale?.[1] ?? 1) === 1;
    if (!identity) {
      console.info("[points] 회전/스케일된 레이어는 점 편집 미지원:", layer.name);
      return;
    }
    this.app.select(id);
    this._editPts = { id };
    this._drawOverlay();
  }

  /** 패널 버튼에서 점 편집 진입(도구가 select가 아니면 먼저 전환). */
  editPointsById(id) {
    const l = this.app.layers().find((v) => v.id === id);
    if (!l) return;
    if ((this.toolState?.tool ?? "select") !== "select") {
      this.dispatchEvent(new CustomEvent("draw-finished", { bubbles: true, composed: true }));
    }
    this._enterPointEdit(id, l, this.app.metaOf(l));
  }

  _exitPointEdit() {
    if (!this._editPts) return;
    this._editPts = null;
    if (this.app.renderer.excludeId != null) {
      this.app.renderer.excludeId = null;
      this.app.renderer.markDirty();
    }
    this._drawOverlay();
  }

  /** 점 편집 지오메트리 — 앵커 item 좌표·화면 좌표·item↔world 변환(identity 전제).
   *  item→world = item − origin(items) + offset (editor-coordinate-contracts). */
  _pointGeom() {
    const ep = this._editPts;
    if (!ep) return null;
    const l = this.app.layers().find((v) => v.id === ep.id);
    const meta = l ? this.app.metaOf(l) : null;
    const pts = l ? this.app.shapePoints(l, meta) : null;
    if (!pts || pts.length < 2) return null;
    const oc = this.app._isDocSizedSurface(l)
      ? [0, 0]
      : this.app._itemsOrigin(this.app.itemsFromMeta(meta)) ?? [0, 0];
    const [ox, oy] = l.offset ?? [0, 0];
    const toWorld = (px, py) => ({ x: px - oc[0] + ox, y: py - oc[1] + oy });
    const toItem = (wx, wy) => [wx - ox + oc[0], wy - oy + oc[1]];
    const screen = [];
    for (let i = 0; i + 1 < pts.length; i += 2) screen.push(this._screen(toWorld(pts[i], pts[i + 1])));
    return { l, meta, pts, screen, toWorld, toItem, kind: meta.item?.shape ?? null };
  }

  /** pts(kind별)를 화면 경로로 구성 — 스트로크/필은 호출자가. map = (item x,y) → 화면점. */
  _tracePointPath(o, pts, kind, map) {
    o.beginPath();
    const flat = kind === "curve" ? curveFlatten(pts) : pts;
    if (flat.length < 4) return;
    const p0 = map(flat[0], flat[1]);
    o.moveTo(p0.x, p0.y);
    for (let i = 2; i < flat.length; i += 2) {
      const p = map(flat[i], flat[i + 1]);
      o.lineTo(p.x, p.y);
    }
    if (kind === "polygon" || kind === "polygon_path") o.closePath();
  }

  _drawAnchors(o, screen, acc, hot = -1) {
    for (let i = 0; i < screen.length; i++) {
      const p = screen[i];
      o.beginPath(); o.arc(p.x, p.y, hot === i ? 4.5 : 3.5, 0, 7);
      o.fillStyle = hot === i ? acc : "#fff"; o.fill();
      o.strokeStyle = acc; o.lineWidth = 1.5; o.stroke();
    }
  }

  /** 점 편집 크롬(드래그 아님) — 도형 윤곽(액센트 얇은 선) + 앵커 핸들. */
  _drawPointEditChrome(o) {
    const g = this._pointGeom();
    if (!g) { this._exitPointEdit(); return; }
    const { acc } = this._themeColors();
    this._tracePointPath(o, g.pts, g.kind, (px, py) => this._screen(g.toWorld(px, py)));
    o.strokeStyle = acc; o.lineWidth = 1;
    o.stroke();
    this._drawAnchors(o, g.screen, acc);
  }

  /** 점 드래그 ghost — 원본 래스터는 excludeId로 숨긴 채 작업 점으로 도형 전체를 미리보기. */
  _drawPointGhost(d) {
    const o = this._overlayCtx(true);
    const z = this._zoom;
    this._drawFrames(o, z);
    const g = d.geom, meta = g.meta;
    const it = meta.item ?? {};
    const map = (px, py) => this._screen(g.toWorld(px, py));
    const opa = g.l?.opacity ?? 1;
    const [cr, cg, cb, ca] = it.rgba ?? meta.rgba ?? [13, 153, 255, 255];
    this._tracePointPath(o, d.pts, g.kind, map);
    if (g.kind === "line" || g.kind === "curve") {
      o.strokeStyle = `rgba(${cr},${cg},${cb},${(ca / 255) * opa})`;
      o.lineWidth = Math.max(1, (it.width ?? 4) * z);
      o.lineCap = "round"; o.lineJoin = "round";
      o.stroke();
    } else {
      if (!meta.noFill) {
        o.fillStyle = `rgba(${cr},${cg},${cb},${(ca / 255) * opa})`;
        o.fill();
      }
      const sw = Number(meta.strokeWidth) || 0;
      if (sw > 0 && meta.stroke) {
        const [sr, sg, sb, sa] = meta.stroke;
        o.strokeStyle = `rgba(${sr},${sg},${sb},${((sa ?? 255) / 255) * opa})`;
        o.lineWidth = sw * z; o.lineJoin = "round";
        o.stroke();
      }
    }
    const { acc } = this._themeColors();
    const screen = [];
    for (let i = 0; i + 1 < d.pts.length; i += 2) screen.push(map(d.pts[i], d.pts[i + 1]));
    this._drawAnchors(o, screen, acc, d.idx);
  }

  /** 도형/곡선 하나를 그리면: 새 레이어 선택 + 선택 도구로 복귀(Figma 동작). */
  _finishDraw(res) {
    const id = res?.bindings?.drawn?.node;
    if (id != null && res?.ok !== false) this.app.select(id);
    this.dispatchEvent(new CustomEvent("draw-finished", { bubbles: true, composed: true }));
    this._drawOverlay();
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
    const font = t.font ?? s.font ?? null;
    const meta = JSON.stringify({ type: "text", x: t.x, y: t.y, text: v, size, rgba, ...(font ? { font } : {}) });
    const name = v.split("\n")[0].slice(0, 20);
    if (t.editId != null) {
      // 기존 텍스트 편집: 노드 보존 재래스터(replace_paint_source) — 그룹 소속·z순서·
      // 선택이 그대로 유지된다. offset은 새 표면 origin 기준으로 리베이스
      // (레거시/이동 레이어 좌상단 점프 방지).
      const xf = t.xf ?? { offset: [0, 0], scale: [1, 1], rotation: 0 };
      const metaObj = { type: "text", x: t.x, y: t.y, text: v, size, rgba, ...(font ? { font } : {}), ...(t.bg ? { bg: t.bg } : {}) };
      const items = this.app.itemsFromMeta(metaObj);
      const pseudo = { offset: xf.offset, surface_size: xf.surface_size };
      const oldItems = this.app.itemsFromMeta({ ...metaObj, text: t.origText ?? v, size: t.origSize ?? size });
      const offset = this.app._rebasedOffset(pseudo, oldItems, items);
      const res = this.app.apply([
        B.replacePaintSource(t.editId, B.shapes(items)),
        B.setProps(t.editId, { name, meta: JSON.stringify(metaObj), offset, scale: xf.scale, rotation: xf.rotation }),
      ]);
      if (res?.ok === false) console.error("텍스트 편집 실패:", res.issues);
      this.app.select(t.editId);
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
    const o = this._overlayCtx();
    const z = this._zoom;
    o.clearRect(0, 0, this._overlayW || this.clientWidth, this._overlayH || this.clientHeight);
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
      case "polygon": {
        const pts = polygonPts(bx + bw / 2, by + bh / 2, bw / 2, bh / 2, s.sides ?? 5);
        o.beginPath();
        o.moveTo(pts[0], pts[1]);
        for (let i = 2; i < pts.length; i += 2) o.lineTo(pts[i], pts[i + 1]);
        o.closePath(); o.fill(); o.stroke();
        break;
      }
      default: o.setLineDash([]); o.lineWidth = s.width * z; o.beginPath(); o.moveTo(s0.x, s0.y); o.lineTo(s1.x, s1.y); o.stroke();
    }
    o.setLineDash([]);
  }

  /** 곡선 진행 ghost — 찍은 앵커(+커서)를 지나는 CR 미리보기 + 앵커 마커. */
  _drawCurveGhost() {
    const o = this._overlayCtx(true);
    const z = this._zoom;
    this._drawFrames(o, z);
    const d = this._drag, s = this.toolState;
    if (d?.mode !== "curve") return;
    const pts = [...d.pts];
    if (d.cur) {
      const lx = pts[pts.length - 2], ly = pts[pts.length - 1];
      if (Math.hypot(d.cur.x - lx, d.cur.y - ly) > 0.5 / (z || 1)) pts.push(d.cur.x, d.cur.y);
    }
    const [r, g, b, a] = s.rgba;
    if (pts.length >= 4) {
      const flat = curveFlatten(pts);
      o.strokeStyle = `rgba(${r},${g},${b},${a / 255})`;
      o.lineWidth = s.width * z;
      o.lineCap = "round"; o.lineJoin = "round";
      o.beginPath();
      const p0 = this._screen({ x: flat[0], y: flat[1] });
      o.moveTo(p0.x, p0.y);
      for (let i = 2; i < flat.length; i += 2) {
        const p = this._screen({ x: flat[i], y: flat[i + 1] });
        o.lineTo(p.x, p.y);
      }
      o.stroke();
    }
    const { acc } = this._themeColors();
    for (let i = 0; i < d.pts.length; i += 2) {
      const p = this._screen({ x: d.pts[i], y: d.pts[i + 1] });
      o.beginPath(); o.arc(p.x, p.y, 3.5, 0, 7);
      o.fillStyle = "#fff"; o.fill();
      o.strokeStyle = acc; o.lineWidth = 1.5; o.stroke();
    }
  }
  /** 브러시 진행 중 폴리라인 ghost. */
  _drawBrushGhost() {
    const o = this._overlayCtx();
    const z = this._zoom;
    o.clearRect(0, 0, this._overlayW || this.clientWidth, this._overlayH || this.clientHeight);
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

  /** 테마 색 캐시 — getComputedStyle은 강제 스타일 재계산이라 드로우 루프에서 반복 금지. */
  _themeColors() {
    const key = document.documentElement.dataset.theme || "";
    if (this._themeCacheKey !== key || !this._themeCache) {
      const cs = getComputedStyle(this);
      const acc = cs.getPropertyValue("--accent").trim() || "#87b9cf";
      this._themeCache = {
        fg: cs.getPropertyValue("--fg-2").trim() || "#555",
        acc,
        accStrong: cs.getPropertyValue("--accent-strong").trim() || acc,
      };
      this._themeCacheKey = key;
    }
    return this._themeCache;
  }

  /** Frame 외곽선 + 이름 라벨(항상 표시 — Figma의 캔버스). */
  _drawFrames(o, z) {
    const preview = this._previewFrame();
    const frames = (this.app.frames?.() ?? []).map((f) => preview && f.id === preview.id ? preview : f);
    const { fg, acc, accStrong } = this._themeColors();
    o.save();
    o.font = "600 11px Inter, sans-serif";
    o.textBaseline = "alphabetic";
    for (const f of frames) {
      let frameColor = fg;
      o.strokeStyle = frameColor; o.lineWidth = 1; o.setLineDash([]);
      const active = this.app.selectedFrameId === f.id;
      const hover = this._hoverFrameId === f.id;
      const p = this._screen({ x: f.x, y: f.y });
      const x = Math.round(p.x) + 0.5;
      const y = Math.round(p.y) + 0.5;
      const w = Math.round(f.w * z);
      const h = Math.round(f.h * z);
      if (active) {
        o.fillStyle = "rgba(135,185,207,0.08)";
        o.fillRect(x, y, w, h);
        frameColor = accStrong;
        o.strokeStyle = frameColor;
        o.lineWidth = 1.5;
      } else if (hover) {
        frameColor = acc;
        o.strokeStyle = frameColor;
        o.lineWidth = 1.5;
      }
      o.strokeRect(x, y, w, h);
      o.fillStyle = frameColor;
      o.fillText(f.name, Math.round(p.x), Math.round(p.y - 5));
      if (active) this._drawFrameHandles(o, this._frameGeom(f), accStrong);
    }
    // 프레임 드래그 중 미리보기.
    const d = this._drag;
    if (d?.mode === "frame") {
      const s0 = this._screen(d.start), s1 = this._screen(d.cur);
      const x = Math.min(s0.x, s1.x), y = Math.min(s0.y, s1.y);
      const w = Math.abs(d.cur.x - d.start.x) * z, h = Math.abs(d.cur.y - d.start.y) * z;
      o.setLineDash([5, 4]);
      o.strokeStyle = fg;
      o.strokeRect(Math.round(x) + 0.5, Math.round(y) + 0.5, Math.round(w), Math.round(h));
      o.setLineDash([]);
    }
    o.restore();
  }

  _drawOverlay() {
    if (!this.overlay) return;
    const o = this._overlayCtx(true);
    this._drawFrames(o, this._zoom);
    // 새 텍스트 입력 중에만 크롬 숨김. 기존 텍스트 편집은 원래 보이던 선택 박스를
    // 그대로 유지한 채 내용만 고친다(더블클릭 시 "다른 핸들로 바뀜" 방지).
    if (this._text && this._text.editId == null) return;
    if (this._drag?.mode === "draw") { this._drawGhost(); return; }
    if (this._drag?.mode === "brush") { this._drawBrushGhost(); return; }
    if (this._drag?.mode === "point") { this._drawPointGhost(this._drag); return; }
    if (this._editPts) { this._drawPointEditChrome(o); return; }
    const acc = this._themeColors().accStrong;
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
    const movePreview = this._drag?.mode === "move" ? this._drag : this._pendingMove;
    if (movePreview?.overlayGeoms) {
      const sx = movePreview.dx * this._zoom;
      const sy = movePreview.dy * this._zoom;
      for (const g of movePreview.overlayGeoms) {
        this._drawSelectionGeom(o, g, acc, single, sx, sy);
      }
    } else {
      for (const sel of this.app.selectedLayers?.() ?? []) {
        const g = this._selGeomFor(sel);
        if (g) this._drawSelectionGeom(o, g, acc, single);
      }
    }
    // 스냅 가이드(move 드래그가 스냅 중일 때만) — 1px 빨간 라인, 캔버스 전체 길이.
    const gd = this._drag?.mode === "move" ? this._drag.guides : null;
    if (gd) {
      const z = this._zoom;
      o.strokeStyle = "#f24822"; o.lineWidth = 1;
      for (const gx of gd.x) {
        const px = Math.round((gx - this._origin.x) * z) + 0.5;
        o.beginPath(); o.moveTo(px, 0); o.lineTo(px, this._overlayH || this.clientHeight); o.stroke();
      }
      for (const gy of gd.y) {
        const py = Math.round((gy - this._origin.y) * z) + 0.5;
        o.beginPath(); o.moveTo(0, py); o.lineTo(this._overlayW || this.clientWidth, py); o.stroke();
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
          const source = t.box ? { x: t.box.x, y: t.box.y } : { x: t.x, y: t.y };
          const pos = xf
            ? this.app.xformOf({ offset: xf.offset, scale: [1, 1], rotation: xf.rotation, surface_size: xf.surface_size }).fwd(source.x, source.y)
            : source;
          const sp = this._screen(pos);
          const rotDeg = xf?.rotation ?? 0;
          return html`<textarea class="txt" spellcheck="false"
          style="left:${sp.x}px; top:${sp.y}px; font-size:${(t.size ?? s?.size ?? 32) * z}px; color:${HEX(t.rgba ?? s?.rgba ?? [13, 153, 255, 255])}; font-family: '${(t.font ?? s?.font ?? "Pretendard").replace(/'/g, "")}', Pretendard, Inter, sans-serif; transform: rotate(${rotDeg}deg); transform-origin: 0 0;"
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
    _collapsed: { state: true }, _projects: { state: true },
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
      display: flex; align-items: center; gap: 6px;
    }
    .project-name .dot {
      width: 6px; height: 6px; border-radius: 50%; background: #43d17a; flex: none;
    }
    .project-menu .pm-label {
      font-size: 9.5px; font-weight: 600; color: var(--fg-3); text-transform: uppercase;
      letter-spacing: 0.5px; padding: 3px 8px 2px;
    }
    .project-menu .pm-proj { gap: 7px; }
    .project-menu .pm-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .project-menu .pm-open { font-size: 9px; color: var(--fg-3); flex: none; }
    .project-menu .pm-proj:hover .pm-open { color: rgba(255,255,255,0.7); }
    .project .add { width: 28px; height: 28px; padding: 0; justify-content: center; flex: none; }
    .list { flex: 1; overflow-y: auto; padding: 0 6px 8px; position: relative; }
    .menu, .ctx, .project-menu {
      position: absolute; right: 10px; top: 2px; z-index: 40; background: var(--bg-panel);
      border: 1px solid var(--line); border-radius: 9px; padding: 5px; min-width: 160px;
      box-shadow: var(--shadow-menu); display: flex; flex-direction: column; gap: 1px;
    }
    .project-menu { top: 38px; min-width: 180px; max-height: 60vh; overflow-y: auto; }
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
    .tri {
      width: 14px; height: 14px; padding: 0; flex: none;
      display: inline-flex; align-items: center; justify-content: center;
      color: var(--fg-3); border-radius: 3px;
    }
    .tri:hover { color: var(--fg); background: var(--bg-hover); }
    .tri-sp { width: 14px; flex: none; }
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
    this._collapsed = new Set(); // 접힌 그룹/프레임 키("n<id>"|"f<id>") — 세션 상태.
    this._projects = null; // /projects 목록(드롭다운 전환용) — 열릴 때 1회 로드.
  }

  /** 현재 문서 id(?doc=) — 프로젝트 폴더와 매칭하는 키. */
  get _docId() {
    return new URLSearchParams(location.search).get("doc") || "";
  }

  /** /projects 폴더를 읽어 현재 문서와 매칭되는 엔트리를 찾는다(드롭다운 표시·전환용). */
  async _loadProjects() {
    try {
      const r = await fetch("/projects");
      if (!r.ok) throw new Error(String(r.status));
      const list = await r.json();
      // 이름 오름차순(서버도 정렬하지만 방어적으로).
      list.sort((a, b) => a.name.localeCompare(b.name));
      this._projects = list;
    } catch {
      this._projects = []; // 로컬/오프라인 모드 — 드롭다운 전환 없이 현재 이름만.
    }
  }

  _toggleCollapse(key) {
    const next = new Set(this._collapsed);
    if (next.has(key)) next.delete(key); else next.add(key);
    this._collapsed = next;
  }

  /** 캔버스에서 선택된 레이어가 접힌 그룹/프레임 안에 있으면 펼친다(Figma reveal). */
  _revealSelection() {
    const sel = new Set(this.app?.selectedIds ?? []);
    if (!sel.size || !this._collapsed.size) return;
    const next = new Set(this._collapsed);
    let changed = false;
    const visit = (l, path) => {
      if (sel.has(l.id)) {
        for (const k of path) if (next.delete(k)) changed = true;
      }
      for (const c of l.children ?? []) visit(c, [...path, `n${l.id}`]);
    };
    for (const item of this._items()) {
      if (item.itemType === "frame") {
        for (const c of item.children ?? []) visit(c, [`f${item.id}`]);
      } else {
        visit(item, []);
      }
    }
    if (changed) this._collapsed = next;
  }
  connectedCallback() {
    super.connectedCallback();
    this._onChange = () => { this._revealSelection(); this._v++; };
    this.app?.addEventListener("changed", this._onChange);
    this._onDoc = (e) => {
      if ((this._menu || this._ctx || this._projectMenu) && !e.composedPath().includes(this)) {
        this._menu = false;
        this._ctx = null;
        this._projectMenu = false;
      }
    };
    document.addEventListener("pointerdown", this._onDoc);
    // 문서 열림 시 폴더 확인 — 현재 문서와 매칭되는 프로젝트를 드롭다운에 표시.
    this._loadProjects();
  }
  disconnectedCallback() {
    this.app?.removeEventListener("changed", this._onChange);
    document.removeEventListener("pointerdown", this._onDoc);
    super.disconnectedCallback();
  }
  _addLayer(color) {
    this._menu = false;
    if (!color) {
      this.app.apply([B.addPaintLayer("layer", B.transparent())]);
      return;
    }
    // 단색 레이어는 벡터 rect로 — 뷰 합성이 전 배율에서 재래스터(샘플링 비용·계단 제거).
    const W = this.app.editor.width(), H = this.app.editor.height();
    const item = B.rect(0, 0, W, H, color);
    this.app.apply([
      B.addPaintLayer("fill", B.shapes([item]), { bind: "fill" }),
      B.setProps("fill", { meta: JSON.stringify({ type: "shape", shape: "rect", item, fill: color, rgba: color, stroke: null, strokeWidth: 0 }) }),
    ]);
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
  async _projectAction(fn) {
    this._projectMenu = false;
    // 네비게이션/이름변경 전 진행 중 편집을 데몬에 도달시킨다(마지막 편집 유실 방지).
    await this.app?.live?.flush?.();
    fn();
  }
  /** 다른 프로젝트로 전환. 진행 중 편집 flush는 호출부(_projectAction)에서 끝난 뒤다 —
   *  데몬은 도달한 편집을 in-memory 보존 + 1.5s 디바운스로 디스크 저장하므로 이동해도 안전. */
  _switchProject(name) {
    if (!name || name === this._docId) return;
    location.search = `?doc=${encodeURIComponent(name)}`;
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
    const meta = this.app.metaOf(l);
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
    const colKey = isFrame ? `f${l.id}` : `n${l.id}`;
    const hasKids = (l.children?.length ?? 0) > 0;
    // 검색 중엔 매치 가시성을 위해 강제 펼침.
    const open = this._query.trim() ? true : !this._collapsed.has(colKey);
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
        ${hasKids
          ? html`<button class="tri" title=${open ? "접기" : "펼치기"}
              @click=${(e) => { e.stopPropagation(); this._toggleCollapse(colKey); }}>${icon(open ? "chevDown" : "chevRight", 9)}</button>`
          : html`<span class="tri-sp"></span>`}
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
      ${open ? (l.children ?? []).slice().reverse().map((child) => this._row(child, depth + 1, selIds)) : nothing}
    `;
  }
  render() {
    const items = this._filterItems(this._items(), this._query);
    const selIds = this.app?.selectedIds ?? [];
    const docId = this._docId;
    // 폴더에서 현재 문서와 매칭되는 프로젝트 — 있으면 그 이름, 없으면 doc id 폴백.
    const matched = (this._projects ?? []).find((p) => p.name === docId);
    const projectName = matched?.name || docId || "Untitled";
    const others = (this._projects ?? []).filter((p) => p.name !== docId);
    return html`
      <div class="project">
        <div class="project-name" title=${matched ? `${projectName} (열림)` : projectName}>
          ${projectName}${matched ? html`<span class="dot" title="폴더에 저장됨"></span>` : nothing}
        </div>
        <button class="add" title="프로젝트 전환·관리"
          @click=${(e) => { e.stopPropagation(); this._projectMenu = !this._projectMenu; if (this._projectMenu) this._loadProjects(); }}>
          ${icon("chevDown", 13)}
        </button>
        ${this._projectMenu ? html`
          <div class="project-menu">
            ${others.length ? html`
              <div class="pm-label">프로젝트 전환</div>
              ${others.map((p) => html`
                <button class="pm-proj" @click=${() => this._projectAction(() => this._switchProject(p.name))}
                  title=${p.modified ? `수정: ${p.modified}` : p.name}>
                  ${icon("folder", 13)}<span class="pm-name">${p.name}</span>${p.open ? html`<span class="pm-open">열림</span>` : nothing}
                </button>`)}
              <div class="hr"></div>
            ` : nothing}
            <button @click=${() => this._projectAction(() => { location.href = "/"; })}>${icon("chevLeft", 13)}대시보드</button>
            <button @click=${() => this._projectAction(() => this._renameProject(projectName))}>${icon("text", 13)}이름 변경</button>
            <div class="hr"></div>
            <button @click=${() => this._projectAction(() => this.dispatchEvent(new CustomEvent("export-png", { bubbles: true, composed: true })))}>${icon("export", 13)}전체 PNG</button>
            <button @click=${() => this._projectAction(() => this.dispatchEvent(new CustomEvent("save-psd", { bubbles: true, composed: true })))}>${icon("export", 13)}PSD 내보내기</button>
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
  static properties = { app: { attribute: false }, _v: { state: true }, _lock: { state: true }, _fonts: { state: true } };
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
    .sec-t { font-size: 10.5px; font-weight: 600; color: var(--fg); margin-bottom: 10px;
             display: flex; align-items: center; gap: 6px; }
    .sec-t .sp { flex: 1; }
    .sec-t .b, .rowbtns .b { width: 24px; height: 24px; padding: 0; justify-content: center; }
    .rowbtns { display: flex; gap: 2px; align-items: center; justify-content: flex-end; }
    .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: 7px; }
    .grid3 { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 7px; }
    .cell {
      display: flex; align-items: center; gap: 0; min-width: 0; height: 28px;
      background: var(--bg-elev); border-radius: var(--radius); border: 1px solid transparent;
    }
    .cell:focus-within { border-color: var(--accent); }
    .cell span {
      padding: 0 0 0 8px; color: var(--fg-3); font-size: 10px;
      flex: none; white-space: nowrap;
    }
    .cell input {
      background: none; border: none; width: 100%; min-width: 0;
      height: 100%; padding: 0 8px 0 5px;
    }
    .cell input[type="number"]::-webkit-outer-spin-button,
    .cell input[type="number"]::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }
    .cell input[type="number"] { appearance: textfield; -moz-appearance: textfield; }
    /* 피그마식: 라벨은 컨트롤 위 작은 회색 텍스트(셀 안 긴 라벨 = 줄바꿈 원인) */
    .lbl { font-size: 10px; color: var(--fg-3); margin: 10px 0 5px; }
    .lbl:first-child { margin-top: 0; }
    .lbls { display: grid; grid-template-columns: 1fr 1fr; gap: 7px; margin: 10px 0 5px; }
    .lbls.three { grid-template-columns: 1fr 1fr 1fr; }
    .lbls .lbl { margin: 0; }
    .alignr { display: flex; gap: 1px; justify-content: space-between; }
    .alignr button { width: 32px; height: 28px; padding: 0; justify-content: center; }
    .field { margin-top: 10px; }
    .field:first-child { margin-top: 0; }
    .field > label { display: flex; justify-content: space-between; align-items: center; font-size: 10.5px; color: var(--fg-2); margin-bottom: 6px; }
    .field .v { color: var(--fg); }
    .field input[type="range"], .field select { width: 100%; }
    .colorrow { display: grid; grid-template-columns: 34px 1fr; gap: 7px; align-items: center; }
    .colorrow input[type="color"] { width: 34px; padding: 0; overflow: hidden; }
    /* 컨트롤이 연달아 쌓일 때 세로 간격 — 채움(단색 셀렉트↔색상행), 그라데이션 2색 등. */
    select + .colorrow, .colorrow + .colorrow, .colorrow + select, select + select { margin-top: 7px; }
    .chk { display: flex; align-items: center; gap: 8px; font-size: 11px; color: var(--fg-2); cursor: pointer; }
    .ptedit {
      width: 100%; justify-content: flex-start; gap: 6px; margin-top: 10px;
      height: 28px; color: var(--fg-2); background: var(--bg-elev); border-radius: var(--radius);
    }
    .ptedit:hover { background: var(--accent); color: #fff; }
    .empty { padding: 22px 14px; font-size: 11px; color: var(--fg-3); line-height: 1.8; }
  `];
  constructor() { super(); this._v = 0; this._lock = false; this._fonts = null; }

  _loadFonts() {
    if (this._fonts || this._fontsLoading) return;
    this._fontsLoading = true;
    (async () => {
      this._fonts = await this.app?.fontList?.() ?? ["Pretendard"];
      this._fontsLoading = false;
      this.requestUpdate();
    })();
  }
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
    const meta = this.app.metaOf(l);
    const isText = meta?.type === "text";
    const isShape = meta?.type === "shape";
    const isBrush = meta?.type === "brush";
    const canStyleColor = (meta?.type === "text") || ((meta?.type === "shape" || meta?.type === "brush") && meta?.item);
    const styleRgba = isShape ? (meta?.fill ?? meta?.rgba ?? meta?.item?.rgba ?? [13, 153, 255, 255]) : (meta?.rgba ?? meta?.item?.rgba ?? [13, 153, 255, 255]);
    const styleHex = HEX(styleRgba);
    const strokeRgba = meta?.stroke ?? [20, 24, 28, 255];
    const strokeHex = HEX(strokeRgba);
    const strokeWidth = Math.round(meta?.strokeWidth ?? 0);
    const itemShape = String(meta?.item?.shape ?? "").toLowerCase();
    const shapeKind = String(meta?.shape ?? meta?.item?.shape ?? l.name ?? "").toLowerCase();
    const isStrokeKind = isShape && (itemShape === "line" || itemShape === "curve");
    const isPolyPath = isShape && itemShape === "polygon_path";
    const canSides = isShape && itemShape === "polygon";
    // 점 편집 가능(선/곡선/다각형/자유다각형) — 패널에 안내.
    const canPointEdit = isShape && ["line", "curve", "polygon", "polygon_path"].includes(itemShape)
      && (l.rotation ?? 0) === 0 && (l.scale?.[0] ?? 1) === 1 && (l.scale?.[1] ?? 1) === 1;
    const canRadius = isShape && !shapeKind.includes("ellipse") && !canSides && !isPolyPath && !isStrokeKind;
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
      // 도형/브러시는 벡터 재래스터(bake)로 — scale 보간 화질 저하 방지.
      if (this.app.bakeShapeScale(l.id, ns, off)) return;
      this.app.apply([B.setProps(l.id, { scale: ns, offset: off })]);
    };
    const setW = (v) => { if (!isText && b && b[2] > 0 && v > 0) setScaleAnchored([(v / b[2]) * Math.sign(sx || 1), sy], tlAnchor); };
    const setH = (v) => { if (!isText && b && b[3] > 0 && v > 0) setScaleAnchored([sx, (v / b[3]) * Math.sign(sy || 1)], tlAnchor); };
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
    const fmt3 = (n) => Number.isFinite(n) ? String(Math.round(n * 1000) / 1000) : "0";
    const num = (label, value, onChange, disabled = false) => html`
      <div class="cell"><span>${label}</span>
        <input type="number" .value=${String(value)} ?disabled=${disabled} @change=${(e) => onChange(e.target.value)} /></div>`;
    return html`
      <div class="head">Design<span class="nm">· ${l.name}</span>
        <button class="b" title="복제 (Cmd+D)" @click=${() => this.app.duplicate(l.id)}>${icon("dup", 13)}</button>
        <button class="b" title="삭제 (Del)" @click=${() => this.app.apply([B.deleteLayer(l.id)])}>${icon("trash", 13)}</button>
      </div>
      <div class="sec">
        <div class="sec-t">위치</div>
        <div class="lbl">정렬</div>
        <div class="alignr">
          ${["left", "center-h", "right", "top", "center-v", "bottom"].map((m, i) => html`
            <button title=${m} @click=${() => this.app.align(l.id, m)}>
              ${icon(["alignL", "alignCH", "alignR", "alignT", "alignCV", "alignB"][i], 14)}</button>`)}
        </div>
        <div class="lbl">위치</div>
        <div class="grid2">
          ${num("X", absX, (v) => commitXY("x", v))}
          ${num("Y", absY, (v) => commitXY("y", v))}
        </div>
        <div class="lbl">회전</div>
        <div class="grid2">
          ${num("∠", Math.round((l.rotation ?? 0) * 10) / 10, (v) => setRotAnchored(+v || 0))}
          <div class="rowbtns">
            <button class="b" title="90° 회전" @click=${() => setRotAnchored((((l.rotation ?? 0) + 90) % 360 + 360) % 360)}>${icon("rot90", 13)}</button>
            <button class="b" title="좌우 반전 (Shift+H)" @click=${() => this.app.flipMany([l.id], "x")}>${icon("flipH", 13)}</button>
            <button class="b" title="상하 반전 (Shift+V)" @click=${() => this.app.flipMany([l.id], "y")}>${icon("flipV", 13)}</button>
          </div>
        </div>
      </div>
      <div class="sec">
        <div class="sec-t">레이아웃
          <span class="sp"></span>
          <button class="b ${this._lock ? "active" : ""}" title="비율 잠금"
            @click=${() => { this._lock = !this._lock; }}>${icon(this._lock ? "lock" : "unlock", 12)}</button>
        </div>
        <div class="lbl">크기</div>
        <div class="grid2">
          ${num("W", wPx, (v) => {
            const nv = +v;
            if (this._lock && !isText && b && wPx > 0 && nv > 0) {
              const r = nv / wPx;
              setScaleAnchored([sx * r, sy * r], tlAnchor);
            } else setW(nv);
          }, isText)}
          ${num("H", hPx, (v) => {
            const nv = +v;
            if (this._lock && !isText && b && hPx > 0 && nv > 0) {
              const r = nv / hPx;
              setScaleAnchored([sx * r, sy * r], tlAnchor);
            } else setH(nv);
          }, isText)}
        </div>
        <div class="lbl">스케일</div>
        <div class="grid2">
          <div class="cell" style="grid-column:1/3"><span>S</span>
            <input type="text" .value=${`${fmt3(sx)} , ${fmt3(sy)}`} title=${isText ? "텍스트는 폰트 변형 방지를 위해 scale 편집이 비활성화됨" : "scale (x , y)"}
              ?disabled=${isText}
              @change=${(e) => {
                const m = e.target.value.split(",").map((s2) => parseFloat(s2));
                if (m.length === 2 && m.every((n) => Number.isFinite(n) && n !== 0))
                  setScaleAnchored(m, ctrAnchor ?? { x: 0, y: 0 });
              }} /></div>
        </div>
      </div>
      <div class="sec">
        <div class="sec-t">모양
          <span class="sp"></span>
          <button class="b" title=${l.visible ? "숨기기" : "표시"}
            @click=${() => this._set({ visible: !l.visible })}>${icon(l.visible ? "eye" : "eyeOff", 13)}</button>
        </div>
        <div class="lbls">
          <span class="lbl">불투명도</span>
          ${canRadius ? html`<span class="lbl">모서리 반경</span>`
            : canSides ? html`<span class="lbl">변 개수</span>` : html`<span></span>`}
        </div>
        <div class="grid2">
          <div class="cell"><span>%</span>
            <input type="number" min="0" max="100" step="1" .value=${String(Math.round(l.opacity * 100))}
              @change=${(e) => this._set({ opacity: Math.max(0, Math.min(100, +e.target.value || 0)) / 100 })} /></div>
          ${canRadius ? html`
            <div class="cell"><span>⌒</span>
              <input type="number" min="0" max="400" step="1" .value=${String(Math.round(meta.radius ?? meta.item?.radius ?? 0))}
                @change=${(e) => this.app.setShapeRadius(l.id, +e.target.value)} /></div>` : nothing}
          ${canSides ? html`
            <div class="cell" title="다각형 변 개수 (3~64)"><span>변</span>
              <input type="number" min="3" max="64" step="1" .value=${String(Math.round(meta.sides ?? meta.item?.sides ?? 5))}
                @change=${(e) => this.app.setShapeSides(l.id, +e.target.value)} /></div>` : nothing}
        </div>
        ${canPointEdit ? html`
          <button class="ptedit" title="캔버스에서 더블클릭해도 진입합니다"
            @click=${() => this.dispatchEvent(new CustomEvent("edit-points", { detail: l.id, bubbles: true, composed: true }))}>
            ${icon("pencil", 12)}점 편집${canSides ? html` <span style="opacity:.6">(자유 다각형으로 변환)</span>` : nothing}
          </button>` : nothing}
        <div class="lbl">블렌드</div>
        <div class="field" style="margin-top:0">
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
      </div>
      ${(() => {
        // ── 공용 색+알파 한 줄 컨트롤 ──
        // 숫자 입력은 다른 필드(W/X/Y)와 같은 cell+접두 라벨 패턴 — 맨숫자 금지(의미 불명).
        const colorAlpha = (rgba, onChange) => html`
          <div class="colorrow">
            <input type="color" title="색상" .value=${HEX(rgba)}
              @change=${(e) => onChange(RGBA(e.target.value, (rgba[3] ?? 255) / 255))} />
            <div class="cell" title="불투명도 %"><span>%</span>
              <input type="number" min="0" max="100" step="1"
                .value=${String(Math.round(((rgba[3] ?? 255) / 255) * 100))}
                @change=${(e) => { const a = Math.max(0, Math.min(100, +e.target.value || 0)) / 100; onChange([rgba[0], rgba[1], rgba[2], Math.round(a * 255)]); }} />
            </div>
          </div>`;
        // ── 그라데이션 헬퍼(bbox 상대 0~1, 2-stop) ──
        const mkGrad = (deg, radial, c1, c2) => {
          const r = (deg * Math.PI) / 180;
          const dx = Math.cos(r) / 2, dy = Math.sin(r) / 2;
          return { x0: 0.5 - dx, y0: 0.5 - dy, x1: 0.5 + dx, y1: 0.5 + dy, radial, stops: [{ at: 0, rgba: c1 }, { at: 1, rgba: c2 }] };
        };
        const gradAngle = (g) => Math.round(Math.atan2((g?.y1 ?? 1) - (g?.y0 ?? 0), (g?.x1 ?? 0) - (g?.x0 ?? 0)) * 180 / Math.PI);
        const fillEditor = (cur, onApply) => {
          const grad = cur.gradient ?? null;
          const mode = cur.none ? "none" : grad ? (grad.radial ? "radial" : "linear") : "solid";
          const c1 = grad?.stops?.[0]?.rgba ?? cur.rgba ?? [13, 153, 255, 255];
          const c2 = grad?.stops?.[1]?.rgba ?? [255, 255, 255, 255];
          const ang = grad ? gradAngle(grad) : 90;
          const apply = (m, a = ang, k1 = c1, k2 = c2) => {
            if (m === "none") onApply({ kind: "none" });
            else if (m === "solid") onApply({ kind: "solid", rgba: k1 });
            else onApply({ kind: "gradient", gradient: mkGrad(a, m === "radial", k1, k2) });
          };
          return html`
            <select .value=${mode} @change=${(e) => apply(e.target.value)}>
              <option value="solid">단색</option>
              <option value="linear">선형 그라데이션</option>
              <option value="radial">방사형 그라데이션</option>
              ${cur.allowNone ? html`<option value="none">없음</option>` : nothing}
            </select>
            ${mode !== "none" ? colorAlpha(c1, (c) => apply(mode, ang, c, c2)) : nothing}
            ${mode === "linear" || mode === "radial" ? html`
              ${colorAlpha(c2, (c) => apply(mode, ang, c1, c))}
              ${mode === "linear" ? html`
                <div class="lbl">각도</div>
                <div class="grid2">
                  <div class="cell"><span>∠</span>
                    <input type="number" step="15" .value=${String(ang)}
                      @change=${(e) => apply(mode, +e.target.value || 0)} /></div>
                </div>` : nothing}
            ` : nothing}`;
        };
        if (isStrokeKind) {
          // 선/곡선: 채움·테두리 대신 선 색+두께만(외곽 stroke 개념이 없는 스트로크 도형).
          return html`
            <div class="sec">
              <div class="sec-t">선</div>
              ${colorAlpha(styleRgba, (c) => this.app.setLayerColor(l.id, c))}
              <div class="lbl">두께</div>
              <div class="grid2">
                <div class="cell"><span>W</span>
                  <input type="number" min="1" max="400" step="1" .value=${String(Math.round(meta.item?.width ?? 4))}
                    @change=${(e) => this.app.setItemWidth(l.id, +e.target.value)} /></div>
              </div>
            </div>`;
        }
        if (isShape) {
          const shadow = meta?.shadow ?? null;
          const hasStroke = strokeWidth > 0 && meta?.stroke;
          return html`
            <div class="sec">
              <div class="sec-t">채움</div>
              ${fillEditor(
                { rgba: styleRgba, gradient: meta?.item?.gradient ?? null, none: !!meta?.noFill, allowNone: true },
                (spec) => spec.kind === "solid" ? this.app.setLayerColor(l.id, spec.rgba) : this.app.setShapeFill(l.id, spec),
              )}
            </div>
            <div class="sec">
              <div class="sec-t">테두리
                <span class="sp"></span>
                ${hasStroke
                  ? html`<button class="b" title="테두리 제거" @click=${() => this.app.setShapeStroke(l.id, null, 0)}>${icon("minus", 12)}</button>`
                  : html`<button class="b" title="테두리 추가" @click=${() => this.app.setShapeStroke(l.id, [20, 24, 28, 255], 3)}>${icon("plus", 12)}</button>`}
              </div>
              ${hasStroke ? html`
                ${colorAlpha(strokeRgba, (c) => this.app.setShapeStroke(l.id, c, Math.max(1, strokeWidth || 1)))}
                <div class="lbl">두께</div>
                <div class="grid2">
                  <div class="cell"><span>W</span>
                    <input type="number" min="0" max="400" step="1" .value=${String(strokeWidth)}
                      @change=${(e) => this.app.setShapeStroke(l.id, strokeRgba, +e.target.value)} /></div>
                </div>
              ` : nothing}
            </div>
            <div class="sec">
              <div class="sec-t">효과
                <span class="sp"></span>
                ${shadow
                  ? html`<button class="b" title="그림자 제거" @click=${() => this.app.setShapeShadow(l.id, null)}>${icon("minus", 12)}</button>`
                  : html`<button class="b" title="그림자 추가" @click=${() => this.app.setShapeShadow(l.id, { dx: 0, dy: 8, blur: 24, rgba: [10, 14, 20, 110] })}>${icon("plus", 12)}</button>`}
              </div>
              ${shadow ? html`
                <div class="lbls three">
                  <span class="lbl">X</span><span class="lbl">Y</span><span class="lbl">흐림</span>
                </div>
                <div class="grid3">
                  <div class="cell"><input type="number" .value=${String(shadow.dx ?? 0)}
                    @change=${(e) => this.app.setShapeShadow(l.id, { ...shadow, dx: +e.target.value || 0 })} /></div>
                  <div class="cell"><input type="number" .value=${String(shadow.dy ?? 8)}
                    @change=${(e) => this.app.setShapeShadow(l.id, { ...shadow, dy: +e.target.value || 0 })} /></div>
                  <div class="cell"><input type="number" min="0" .value=${String(shadow.blur ?? 24)}
                    @change=${(e) => this.app.setShapeShadow(l.id, { ...shadow, blur: Math.max(0, +e.target.value || 0) })} /></div>
                </div>
                <div class="lbl">색상</div>
                ${colorAlpha(shadow.rgba ?? [10, 14, 20, 110], (c) => this.app.setShapeShadow(l.id, { ...shadow, rgba: c }))}
              ` : nothing}
            </div>`;
        }
        if (isText) {
          const bg = meta?.bg ?? null;
          return html`
            <div class="sec">
              <div class="sec-t">텍스트</div>
              <div class="lbl">글꼴</div>
              <div class="field" style="margin-top:0">
                ${this._loadFonts() ?? nothing}
                <select .value=${meta.font ?? "Pretendard"} title="글꼴"
                  @change=${async (e) => {
                    const v = e.target.value;
                    if (await this.app.ensureFont(v) === false) {
                      alert(`글꼴을 불러오지 못했습니다: ${v}`);
                      return;
                    }
                    this.app.setTextFont(l.id, v);
                  }}>
                  ${(this._fonts ?? [meta.font ?? "Pretendard"]).map((f) => html`<option value=${f} ?selected=${f === (meta.font ?? "Pretendard")}>${f}</option>`)}
                </select>
              </div>
              <div class="lbls">
                <span class="lbl">크기</span><span></span>
              </div>
              <div class="grid2">
                <div class="cell"><span>px</span>
                  <input type="number" min="6" max="400" step="1" .value=${String(Math.round(meta.size ?? 32))}
                    @change=${(e) => this.app.setTextSize(l.id, +e.target.value)} /></div>
              </div>
              <div class="lbl">글자색</div>
              ${colorAlpha(styleRgba, (c) => this.app.setLayerColor(l.id, c))}
            </div>
            <div class="sec">
              <div class="sec-t">배경
                <span class="sp"></span>
                ${bg
                  ? html`<button class="b" title="배경 제거" @click=${() => this.app.setTextBg(l.id, null)}>${icon("minus", 12)}</button>`
                  : html`<button class="b" title="배경 추가" @click=${() => this.app.setTextBg(l.id, { rgba: [255, 213, 95, 255] })}>${icon("plus", 12)}</button>`}
              </div>
              ${bg ? html`
                ${fillEditor(
                  { rgba: bg.rgba ?? [255, 213, 95, 255], gradient: bg.gradient ?? null, none: false, allowNone: false },
                  (spec) => {
                    if (spec.kind === "solid") { const nb = { ...bg, rgba: spec.rgba }; delete nb.gradient; this.app.setTextBg(l.id, nb); }
                    else this.app.setTextBg(l.id, { ...bg, gradient: spec.gradient });
                  },
                )}
                <div class="lbls">
                  <span class="lbl">패딩</span><span class="lbl">모서리 반경</span>
                </div>
                <div class="grid2">
                  <div class="cell"><input type="number" min="0" .value=${String(Math.round(bg.padX ?? (meta.size ?? 32) * 0.35))}
                    @change=${(e) => { const v = Math.max(0, +e.target.value || 0); this.app.setTextBg(l.id, { ...bg, padX: v, padY: Math.round(v * 0.63) }); }} /></div>
                  <div class="cell"><input type="number" min="0" .value=${String(Math.round(bg.radius ?? (meta.size ?? 32) * 0.18))}
                    @change=${(e) => this.app.setTextBg(l.id, { ...bg, radius: Math.max(0, +e.target.value || 0) })} /></div>
                </div>
              ` : nothing}
            </div>`;
        }
        if (canStyleColor) {
          return html`
            <div class="sec">
              <div class="sec-t">채움</div>
              ${colorAlpha(styleRgba, (c) => this.app.setLayerColor(l.id, c))}
            </div>`;
        }
        return nothing;
      })()}
      <div class="sec">
        <div class="sec-t">Export
          <span class="sp"></span>
          <button class="b" title="이 레이어만 PNG로" @click=${() => this.app.exportLayerPng(l)}>${icon("download", 13)}</button>
        </div>
      </div>
    `;
  }
}
customElements.define("dx-props", DxProps);

// ───────── 에이전트 터미널 버블 ─────────
class DxAgentTerminal extends LitElement {
  static properties = {
    docId: { attribute: "doc-id" },
    _open: { state: true },
    _active: { state: true },
    _status: { state: true },
    _error: { state: true },
  };
  static styles = [controls, css`
    :host { display: contents; }
    .bubble {
      position: fixed; right: 24px; bottom: 24px; z-index: 96;
      width: 44px; height: 44px; padding: 0; justify-content: center;
      border-radius: 50%; background: var(--accent-strong); color: #fff;
      box-shadow: 0 14px 34px rgba(0, 0, 0, 0.36);
    }
    .bubble:hover { background: var(--accent-strong); color: #fff; filter: brightness(1.08); }
    .bubble .live {
      position: absolute; top: 2px; right: 2px; width: 10px; height: 10px;
      border-radius: 50%; background: #43d17a; border: 2px solid var(--bg-panel);
    }
    .panel {
      position: fixed; right: 24px; bottom: 24px; z-index: 96;
      width: min(760px, calc(100vw - 288px)); height: min(520px, calc(100vh - 82px));
      min-width: 420px; min-height: 300px; display: grid; grid-template-rows: auto 1fr;
      background: #101316; color: #e9eef2; border: 1px solid var(--line);
      border-radius: 10px; overflow: hidden; box-shadow: 0 20px 56px rgba(0, 0, 0, 0.48);
    }
    .head {
      height: 44px; display: flex; align-items: center; gap: 8px;
      padding: 0 8px 0 10px; background: var(--bg-panel); border-bottom: 1px solid var(--line);
    }
    .mark { color: var(--accent); display: flex; flex: none; }
    .tabs { display: flex; align-items: center; gap: 2px; flex: 1; min-width: 0; }
    .tabs button { height: 30px; color: var(--fg-2); }
    .tabs button.active { background: var(--accent-soft); color: var(--fg); }
    .guide {
      flex: none; height: 30px; display: inline-flex; align-items: center; padding: 0 8px;
      color: var(--fg-2); text-decoration: none; border-radius: var(--radius);
    }
    .guide:hover { background: var(--bg-hover); color: var(--fg); }
    .state {
      flex: none; max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
      color: var(--fg-3); font-size: 10.5px;
    }
    .close { width: 30px; height: 30px; padding: 0; justify-content: center; flex: none; }
    .term {
      position: relative; min-height: 0; background: #101316;
    }
    .term-mount { position: absolute; inset: 8px; }
    .idle {
      position: absolute; inset: 0; display: grid; place-content: center; gap: 10px;
      color: var(--fg-3); text-align: center;
    }
    .idle-row { display: flex; gap: 6px; justify-content: center; }
    .idle-row button { color: var(--fg); background: var(--bg-elev); border: 1px solid var(--line); }
    .idle-row button:hover { background: var(--accent); color: #10232c; }
    .err {
      position: absolute; left: 10px; right: 10px; bottom: 10px;
      padding: 7px 9px; border-radius: 7px; color: #ffd5cb;
      background: rgba(242, 72, 34, 0.16); border: 1px solid rgba(242, 72, 34, 0.35);
      font-size: 11px;
    }
    @media (max-width: 760px) {
      .panel { left: 12px; right: 12px; bottom: 12px; width: auto; min-width: 0; height: min(520px, calc(100vh - 24px)); }
      .bubble { right: 16px; bottom: 16px; }
      .state { display: none; }
    }
  `];
  constructor() {
    super();
    this.docId = "";
    this._open = false;
    this._active = null;
    this._status = "idle";
    this._error = "";
    this._encoder = new TextEncoder();
    this._decoder = new TextDecoder();
    // kind → 살아있는 세션({term, fit, ws, mount, ...}). 탭 전환·패널 닫기에도 유지 —
    // 세션은 새로고침(disconnectedCallback)에서만 정리된다.
    this._sessions = new Map();
  }
  disconnectedCallback() {
    this._ro?.disconnect();
    this._ro = null;
    for (const kind of [...this._sessions.keys()]) this._disposeSession(kind);
    super.disconnectedCallback();
  }
  async _ensureXterm() {
    if (!window.__dxXterm) {
      window.__dxXterm = Promise.all([
        import("https://esm.sh/@xterm/xterm@5.5.0"),
        import("https://esm.sh/@xterm/addon-fit@0.10.0"),
      ]);
    }
    const [{ Terminal }, { FitAddon }] = await window.__dxXterm;
    return { Terminal, FitAddon };
  }
  /** 탭 진입 — 살아있는 세션이면 재표시만, 죽었거나 없으면 새로 연결. */
  async _start(kind) {
    this._open = true;
    this._active = kind;
    await this.updateComplete;
    const s = this._sessions.get(kind);
    if (s && s.ws && s.ws.readyState <= WebSocket.OPEN) {
      // 살아있는 세션 — 인터럽트 없이 다시 보여준다(탭 전환/패널 재오픈).
      this._syncMounts();
      this._status = s.status;
      this._error = s.error ?? "";
      try { s.fit.fit(); } catch { /* 표시 직후 race 무해 */ }
      s.term.focus();
      return;
    }
    if (s) this._disposeSession(kind); // exited/closed 세션 잔해 → 새로 시작.
    this._status = "connecting";
    this._error = "";
    this._connect(kind);
  }
  async _connect(kind) {
    const host = this.renderRoot.querySelector(".term");
    if (!host) return;
    try {
      const { Terminal, FitAddon } = await this._ensureXterm();
      // 세션별 mount — Lit 템플릿 밖에서 만들어 탭 전환·패널 닫기에 파괴되지 않는다.
      const mount = document.createElement("div");
      mount.className = "term-mount";
      const link = document.createElement("link");
      link.rel = "stylesheet";
      link.href = "https://unpkg.com/@xterm/xterm@5.5.0/css/xterm.css";
      mount.appendChild(link);
      host.appendChild(mount);

      const cs = getComputedStyle(this);
      const term = new Terminal({
        cursorBlink: true,
        convertEol: false,
        allowProposedApi: false,
        fontFamily: '"SFMono-Regular", Menlo, Consolas, monospace',
        fontSize: 12,
        lineHeight: 1.18,
        scrollback: 6000,
        theme: {
          background: "#101316",
          foreground: "#e9eef2",
          cursor: cs.getPropertyValue("--accent").trim() || "#9fc7da",
          selectionBackground: "rgba(159, 199, 218, 0.26)",
        },
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      term.open(mount);

      const proto = location.protocol === "https:" ? "wss" : "ws";
      const qs = new URLSearchParams({ cols: String(term.cols || 100), rows: String(term.rows || 28) });
      if (this.docId) qs.set("doc", this.docId);
      const ws = new WebSocket(`${proto}://${location.host}/terminal/${encodeURIComponent(kind)}?${qs}`);
      ws.binaryType = "arraybuffer";
      const sess = { kind, term, fit, ws, mount, status: "connecting", error: "", disposables: [] };
      this._sessions.set(kind, sess);
      const setStatus = (status, error = sess.error) => {
        sess.status = status;
        sess.error = error;
        if (this._active === kind) {
          this._status = status;
          this._error = error;
        }
      };
      sess.disposables.push(term.onData((data) => {
        if (ws.readyState === WebSocket.OPEN) ws.send(this._encoder.encode(data));
      }));
      sess.disposables.push(term.onResize(({ cols, rows }) => {
        if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: "resize", cols, rows }));
      }));

      ws.onopen = () => setStatus("running");
      ws.onmessage = (ev) => {
        if (typeof ev.data === "string") {
          this._handleControl(sess, setStatus, ev.data);
          return;
        }
        term.write(this._decoder.decode(ev.data, { stream: true }));
      };
      ws.onerror = () => setStatus("error", "terminal connection failed");
      ws.onclose = () => {
        if (sess.status !== "exited" && sess.status !== "idle") setStatus("closed");
      };

      // 패널 리사이즈 → 보이는(활성) 세션만 fit.
      if (!this._ro) {
        this._ro = new ResizeObserver(() => {
          clearTimeout(this._fitTimer);
          this._fitTimer = setTimeout(() => {
            const a = this._sessions.get(this._active);
            if (!a || !this._open) return;
            try {
              a.fit.fit();
              if (a.ws.readyState === WebSocket.OPEN)
                a.ws.send(JSON.stringify({ type: "resize", cols: a.term.cols, rows: a.term.rows }));
            } catch {
              // teardown 중 리사이즈는 무해.
            }
          }, 60);
        });
      }
      this._ro.observe(host);
      this._syncMounts();
      fit.fit();
      term.focus();
    } catch (e) {
      this._status = "error";
      this._error = `xterm load failed: ${e.message ?? e}`;
    }
  }
  _handleControl(sess, setStatus, text) {
    let msg;
    try { msg = JSON.parse(text); } catch { return; }
    if (msg.type === "error") {
      setStatus("error", msg.message || "terminal error");
      sess.term?.writeln(`\r\n${sess.error}`);
    } else if (msg.type === "exit") {
      setStatus("exited");
      sess.term?.writeln(`\r\n[process exited${msg.code == null ? "" : `: ${msg.code}`}]`);
    }
  }
  /** 세션 mount들을 패널에 다시 붙이고 활성만 표시(나머지는 살아있는 채 숨김). */
  _syncMounts() {
    const host = this.renderRoot.querySelector(".term");
    if (!host) return;
    for (const [kind, s] of this._sessions) {
      if (s.mount.parentNode !== host) host.appendChild(s.mount);
      s.mount.style.display = kind === this._active ? "" : "none";
    }
  }
  updated() {
    if (this._open) this._syncMounts();
  }
  _disposeSession(kind) {
    const s = this._sessions.get(kind);
    if (!s) return;
    this._sessions.delete(kind);
    for (const d of s.disposables) d.dispose?.();
    if (s.ws && s.ws.readyState < WebSocket.CLOSING) s.ws.close();
    s.term?.dispose?.();
    s.mount?.remove();
  }
  /** 패널 닫기 — 세션은 유지(새로고침 전까지 인터럽트 없음). 다시 열면 그대로 복귀. */
  _close() {
    clearTimeout(this._fitTimer);
    this._open = false;
  }
  _label(kind) {
    return kind === "codex" ? "Codex" : kind === "claude" ? "Claude Code" : "Shell";
  }
  render() {
    if (!this._open) {
      const live = [...this._sessions.values()].some((s) => s.ws && s.ws.readyState <= WebSocket.OPEN);
      return html`<button class="bubble" title=${live ? "에이전트 터미널 — 세션 실행 중" : "에이전트 터미널"}
        @click=${() => { this._open = true; }}>
        ${icon("terminal", 18)}${live ? html`<span class="live"></span>` : nothing}
      </button>`;
    }
    const agents = ["codex", "claude", "shell"];
    return html`
      <section class="panel" @pointerdown=${(e) => e.stopPropagation()}>
        <div class="head">
          <span class="mark">${icon("terminal", 15)}</span>
          <div class="tabs">
            ${agents.map((kind) => html`
              <button class=${this._active === kind ? "active" : ""} @click=${() => this._start(kind)}>
                ${kind === "shell" ? icon("terminal", 13) : icon("play", 12)}${this._label(kind)}
              </button>`)}
          </div>
          <a class="guide" href=${`/terminal/guide.md${this.docId ? `?doc=${encodeURIComponent(this.docId)}` : ""}`}
            target="_blank" rel="noreferrer">CLI guide</a>
          <span class="state">${this._active ? `${this._label(this._active)} · ${this._status}` : this._status}</span>
          <button class="close" title="닫기 (세션은 유지됨)" @click=${() => this._close()}>${icon("close", 14)}</button>
        </div>
        <div class="term">
          ${!this._active ? html`
            <div class="idle">
              <div class="idle-row">
                <button @click=${() => this._start("codex")}>${icon("play", 12)}Codex</button>
                <button @click=${() => this._start("claude")}>${icon("play", 12)}Claude Code</button>
                <button @click=${() => this._start("shell")}>${icon("terminal", 13)}Shell</button>
              </div>
            </div>` : nothing}
          ${this._error ? html`<div class="err">${this._error}</div>` : nothing}
        </div>
      </section>
    `;
  }
}
customElements.define("dx-agent-terminal", DxAgentTerminal);

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
    if (e.shiftKey && e.key === "2") { this._canvas?.zoomCmd("selection"); return; }
    if (!meta) {
      const map = { v: "select", r: "rect", e: "ellipse", p: "polygon", c: "curve", l: "line", t: "text", b: "brush", f: "frame" };
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
        @save-psd=${() => this.dispatchEvent(new CustomEvent("save-psd", { bubbles: true }))}
        @save-dxpkg=${() => this.dispatchEvent(new CustomEvent("save-dxpkg", { bubbles: true }))}></dx-topbar>
      <dx-layer-panel .app=${this.app}
        @export-png=${() => this.dispatchEvent(new CustomEvent("export-png", { bubbles: true }))}
        @save-psd=${() => this.dispatchEvent(new CustomEvent("save-psd", { bubbles: true }))}
        @save-dxpkg=${() => this.dispatchEvent(new CustomEvent("save-dxpkg", { bubbles: true }))}></dx-layer-panel>
      <dx-canvas .app=${this.app} .toolState=${this._tool}
        @zoom-changed=${(e) => { this._zoom = e.detail; }}
        @picked-color=${(e) => { this._topbar?.setColor(e.detail); this._topbar?.finishEyedrop(); }}
        @text-finished=${() => this._topbar?.setTool("select")}
        @draw-finished=${() => this._topbar?.setTool("select")}
        @edit-points=${(e) => this._canvas?.editPointsById(e.detail)}></dx-canvas>
      <dx-props .app=${this.app} @edit-points=${(e) => this._canvas?.editPointsById(e.detail)}></dx-props>
      <dx-agent-terminal .docId=${new URLSearchParams(location.search).get("doc") || ""}></dx-agent-terminal>
    `;
  }
}
customElements.define("app-shell", AppShell);
