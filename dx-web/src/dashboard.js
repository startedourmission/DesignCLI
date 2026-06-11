// 프로젝트 관리 대시보드 — <dx-dashboard> Lit 컴포넌트.
//
// GET /projects 로 목록을 가져와 카드 그리드로 표시.
// "새 프로젝트" 카드: 인라인 폼으로 이름/폭/높이 입력 → POST /doc/:name/create → 이동.
// 카드 hover 시 삭제 버튼(SVG 휴지통) → DELETE /projects/:name.
// ?doc= 쿼리 없이 접근하면 이 대시보드만 표시(wasm 초기화 불필요).
//
// 디자인 규칙: 이모지 금지, 아이콘 전부 인라인 SVG, 기존 토큰 변수 사용.

import { LitElement, html, css, svg, nothing } from "lit";

// ── 인라인 SVG 아이콘 (components.js icon() 패턴 동일) ──
const P = {
  trash: svg`<path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.5 8.5h6l.5-8.5M6.5 7v4M9.5 7v4"/>`,
  plus:  svg`<path d="M8 3v10M3 8h10"/>`,
  folder: svg`<path d="M1.5 4.5a1 1 0 011-1h3.2l1.4 1.8h6.4a1 1 0 011 1v5.7a1 1 0 01-1 1h-11a1 1 0 01-1-1z"/>`,
};
const icon = (name, size = 15) => svg`
  <svg viewBox="0 0 16 16" width=${size} height=${size} fill="none"
    stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"
    style="display:block">${P[name]}</svg>`;

// ── 날짜 포맷 — ISO 문자열을 사람이 읽기 쉬운 로컬 표현으로 변환 ──
function fmtDate(iso) {
  if (!iso) return "";
  try {
    return new Date(iso).toLocaleString("ko-KR", {
      year: "numeric", month: "2-digit", day: "2-digit",
      hour: "2-digit", minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

export class DxDashboard extends LitElement {
  static properties = {
    _projects: { state: true },  // ProjectEntry[] | null(로딩 중)
    _error:    { state: true },
    _creating: { state: true },  // 새 프로젝트 폼 표시 여부
    _newName:  { state: true },
    _newW:     { state: true },
    _newH:     { state: true },
    _saving:   { state: true },  // POST 요청 중 여부
    _busyMsg:  { state: true },  // 장시간 작업(PSD 변환 등) 오버레이 메시지
  };

  static styles = css`
    :host {
      color-scheme: light;
      --bg-canvas: #e9e9e6;
      --bg-panel: #f5f5f3;
      --bg-elev: #ffffff;
      --bg-hover: #ececea;
      --line: #d2d2ce;
      --line-soft: #e3e3df;
      --fg: #1e1e1e;
      --fg-2: #555555;
      --fg-3: #8a8a86;
      --accent: #87b9cf;
      --accent-strong: #5c9ab6;
      --accent-soft: rgba(135, 185, 207, 0.18);
      --danger: #d9401a;
      display: flex; flex-direction: column;
      min-height: 100vh; background: var(--bg-canvas);
      color: var(--fg); font-family: "Inter", system-ui, sans-serif;
      font-size: 11.5px; line-height: 1.4;
      -webkit-font-smoothing: antialiased;
    }

    /* ── 헤더 ── */
    .header {
      display: flex; align-items: center; gap: 10px;
      padding: 0 28px; height: 54px;
      background: var(--bg-panel); border-bottom: 1px solid var(--line);
      flex-shrink: 0;
    }
    .logo-dot {
      width: 9px; height: 9px; border-radius: 2px;
      background: var(--accent); flex-shrink: 0;
    }
    .logo-text {
      font-weight: 600; font-size: 13px; color: var(--fg); letter-spacing: 0.2px;
    }
    /* ── 본문 ── */
    .body {
      flex: 1; padding: 32px 28px; overflow-y: auto;
    }
    .body-head {
      display: flex; align-items: center; justify-content: space-between;
      gap: 16px; margin-bottom: 18px;
    }
    .body-title { margin: 0; font-size: 22px; line-height: 1.15; color: var(--fg); }
    .body-copy { margin: 4px 0 0; color: var(--fg-3); }
    .btn-new-file {
      height: 32px; border: none; border-radius: 7px; background: var(--fg);
      color: var(--bg-panel); display: inline-flex; align-items: center; gap: 7px;
      padding: 0 12px; font: inherit; font-weight: 600; cursor: pointer;
    }
    .btn-new-file:hover { background: var(--accent); color: #10232c; }
    .busy {
      position: fixed; inset: 0; z-index: 999; display: flex;
      align-items: center; justify-content: center;
      background: rgba(10, 12, 14, 0.4); backdrop-filter: blur(2px);
    }
    .busy .box {
      display: flex; align-items: center; gap: 10px; padding: 13px 18px;
      max-width: min(560px, 80vw);
      background: var(--bg-panel); color: var(--fg);
      border: 1px solid var(--line); border-radius: 10px;
      font-size: 12.5px; box-shadow: 0 14px 38px rgba(0, 0, 0, 0.4);
    }
    .busy .spin {
      width: 14px; height: 14px; flex: none; border-radius: 50%;
      border: 2px solid var(--accent); border-top-color: transparent;
      animation: dxspin 0.8s linear infinite;
    }
    @keyframes dxspin { to { transform: rotate(360deg); } }
    .section-title {
      font-size: 11px; font-weight: 600; color: var(--fg-3);
      text-transform: uppercase; letter-spacing: 0.6px;
      margin: 0 0 16px;
    }

    /* ── 카드 그리드 ── */
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
      gap: 16px;
    }

    /* ── 프로젝트 카드 ── */
    .card {
      position: relative;
      background: var(--bg-panel); border: 1px solid var(--line);
      border-radius: 8px; overflow: hidden;
      cursor: pointer; transition: border-color .15s, box-shadow .15s;
    }
    .card:hover {
      border-color: var(--accent);
      box-shadow: 0 0 0 2px var(--accent-soft);
    }
    .card:hover .card-del { opacity: 1; }

    /* 썸네일 영역 */
    .thumb {
      width: 100%; aspect-ratio: 4/3; object-fit: contain;
      background: var(--bg-elev); display: block;
    }
    .thumb-placeholder {
      width: 100%; aspect-ratio: 4/3;
      background: var(--bg-elev);
      display: flex; align-items: center; justify-content: center;
      color: var(--fg-3);
    }
    .thumb-placeholder::before {
      content: ""; width: 34px; height: 26px; border: 1px solid var(--line);
      border-radius: 4px; background: var(--bg-panel);
      box-shadow: inset 0 -7px 0 var(--line-soft);
    }

    /* 카드 본문 */
    .card-body {
      padding: 10px 12px 12px;
    }
    .card-name {
      font-weight: 500; font-size: 12px; color: var(--fg);
      white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
      margin-bottom: 4px;
    }
    .card-meta {
      color: var(--fg-3); font-size: 10.5px; line-height: 1.5;
    }

    /* 삭제 버튼 */
    .card-del {
      position: absolute; top: 6px; right: 6px;
      opacity: 0; transition: opacity .15s;
      background: var(--bg-panel); border: 1px solid var(--line);
      border-radius: 6px; width: 26px; height: 26px;
      display: flex; align-items: center; justify-content: center;
      cursor: pointer; color: var(--danger);
    }
    .card-del:hover { background: var(--danger); color: #fff; border-color: var(--danger); }

    /* ── 새 프로젝트 카드 ── */
    .card-new {
      background: var(--bg-panel); border: 1.5px dashed var(--line);
      border-radius: 8px; cursor: pointer;
      display: flex; flex-direction: column; align-items: center;
      justify-content: center; gap: 8px;
      aspect-ratio: unset; min-height: 140px;
      transition: border-color .15s, background .15s;
      color: var(--fg-3);
    }
    .card-new:hover { border-color: var(--accent); background: var(--accent-soft); color: var(--accent); }
    .card-new-label { font-size: 11.5px; font-weight: 500; }

    /* ── 새 프로젝트 인라인 폼 ── */
    .card-form {
      background: var(--bg-panel); border: 1.5px solid var(--accent);
      border-radius: 8px; padding: 14px 14px 12px;
      display: flex; flex-direction: column; gap: 8px;
      box-shadow: 0 0 0 3px var(--accent-soft);
    }
    .card-form label {
      font-size: 10.5px; color: var(--fg-3); display: flex;
      flex-direction: column; gap: 3px;
    }
    .card-form input {
      font: inherit; color: var(--fg); background: var(--bg-elev);
      border: 1px solid transparent; border-radius: 5px;
      height: 26px; padding: 0 7px; outline: none; width: 100%;
    }
    .card-form input:focus { border-color: var(--accent); }
    .card-form .row { display: flex; gap: 6px; }
    .card-form .row label { flex: 1; }
    .card-form .actions { display: flex; gap: 6px; margin-top: 2px; }
    .btn-primary {
      flex: 1; height: 28px; border: none; border-radius: 6px;
      background: var(--accent); color: #fff; font: inherit;
      font-size: 11.5px; font-weight: 500; cursor: pointer;
    }
    .btn-primary:hover { background: var(--accent-strong); }
    .btn-primary:disabled { opacity: 0.5; cursor: default; }
    .btn-cancel {
      height: 28px; padding: 0 10px; border: 1px solid var(--line);
      border-radius: 6px; background: none; color: var(--fg-2);
      font: inherit; font-size: 11.5px; cursor: pointer;
    }
    .btn-cancel:hover { background: var(--bg-hover); }

    /* ── 오류 / 빈 상태 ── */
    .msg { color: var(--fg-3); text-align: center; padding: 40px 0; }
    .err { color: var(--danger); }
  `;

  constructor() {
    super();
    this._projects = null;
    this._error = null;
    this._creating = false;
    this._newName = "";
    this._newW = 800;
    this._newH = 600;
    this._saving = false;
  }

  connectedCallback() {
    super.connectedCallback();
    this._load();
  }

  // ── 데이터 로드 ──
  async _load() {
    try {
      const r = await fetch("/projects");
      if (!r.ok) throw new Error(`서버 오류: ${r.status}`);
      this._projects = await r.json();
      this._error = null;
    } catch (e) {
      this._error = `목록 로드 실패: ${e.message}`;
    }
  }

  // ── 프로젝트 열기 ──
  _open(name) {
    location.search = `?doc=${encodeURIComponent(name)}`;
  }

  // ── 삭제 ──
  async _delete(e, name) {
    e.stopPropagation();
    if (!confirm(`"${name}" 프로젝트를 삭제하시겠습니까? 복구할 수 없습니다.`)) return;
    try {
      const r = await fetch(`/projects/${encodeURIComponent(name)}`, { method: "DELETE" });
      if (!r.ok) throw new Error(`삭제 실패: ${r.status}`);
      this._projects = (this._projects || []).filter((p) => p.name !== name);
    } catch (e) {
      alert(`삭제 실패: ${e.message}`);
    }
  }

  // ── PSD 업로드 → 프로젝트 생성(레이어 보존) ──
  async _importPsd(file) {
    const name = file.name.replace(/\.psd$/i, "").trim() || "imported";
    this._saving = true;
    this._busyMsg = `"${file.name}" 변환 중… (레이어 보존 변환은 파일 크기에 따라 수십 초 걸릴 수 있음)`;
    try {
      const r = await fetch(`/projects/import-psd?name=${encodeURIComponent(name)}`, {
        method: "POST",
        headers: { "content-type": "application/octet-stream" },
        body: file,
      });
      if (!r.ok) throw new Error(await r.text().catch(() => String(r.status)));
      const j = await r.json();
      location.search = `?doc=${encodeURIComponent(j.name)}`;
    } catch (e) {
      alert(`PSD 가져오기 실패: ${e.message}`);
      this._saving = false;
      this._busyMsg = "";
    }
  }

  // ── 새 프로젝트 생성 ──
  async _create() {
    const name = this._newName.trim();
    if (!name) return;
    const w = Math.max(1, Math.min(16384, parseInt(this._newW) || 800));
    const h = Math.max(1, Math.min(16384, parseInt(this._newH) || 600));
    this._saving = true;
    try {
      const r = await fetch(
        `/doc/${encodeURIComponent(name)}/create?w=${w}&h=${h}`,
        { method: "POST" }
      );
      if (!r.ok) {
        const msg = await r.text().catch(() => r.status);
        throw new Error(msg);
      }
      // 생성 완료 → 바로 해당 문서로 이동.
      location.search = `?doc=${encodeURIComponent(name)}`;
    } catch (e) {
      alert(`생성 실패: ${e.message}`);
      this._saving = false;
    }
  }

  // ── 폼 초기화 ──
  _cancelCreate() {
    this._creating = false;
    this._newName = "";
    this._newW = 800;
    this._newH = 600;
  }

  // ── 썸네일 img — 404시 placeholder로 교체 ──
  _thumbTemplate(name) {
    return html`
      <img class="thumb"
        src="/doc/${encodeURIComponent(name)}/thumb.png"
        alt="${name}"
        @error=${(e) => {
          // 404 등 실패 → 회색 placeholder div로 교체.
          const img = e.target;
          const ph = document.createElement("div");
          ph.className = "thumb-placeholder";
          img.replaceWith(ph);
        }}
      />
    `;
  }

  render() {
    return html`
      ${this._busyMsg ? html`
        <div class="busy">
          <div class="box"><span class="spin"></span>${this._busyMsg}</div>
        </div>` : nothing}
      <div class="header">
        <div class="logo-dot"></div>
        <span class="logo-text">DesignCLI</span>
      </div>
      <div class="body">
        <div class="body-head">
          <div>
            <h1 class="body-title">최근 작업</h1>
            <p class="body-copy">로컬 문서와 라이브 캔버스를 한 곳에서 엽니다.</p>
          </div>
          <button class="btn-new-file" @click=${() => { this._creating = true; }}>${icon("plus", 14)}새 프로젝트</button>
          <button class="btn-new-file" title="PSD 파일을 프로젝트로 변환(레이어 보존)"
            @click=${() => this.renderRoot.querySelector("#psd-up").click()}>${icon("download", 14)}PSD 가져오기</button>
          <input id="psd-up" type="file" accept=".psd" style="display:none"
            @change=${(e) => { const f = e.target.files?.[0]; if (f) this._importPsd(f); e.target.value = ""; }} />
        </div>
        <p class="section-title">프로젝트</p>
        ${this._error
          ? html`<p class="msg err">${this._error}</p>`
          : this._projects === null
          ? html`<p class="msg">로딩 중...</p>`
          : html`
            <div class="grid">
              ${this._projects.map((p) => this._cardTemplate(p))}
              ${this._creating
                ? this._formTemplate()
                : this._newCardTemplate()}
            </div>
          `}
      </div>
    `;
  }

  _cardTemplate(p) {
    return html`
      <div class="card" @click=${() => this._open(p.name)}>
        ${this._thumbTemplate(p.name)}
        <div class="card-body">
          <div class="card-name" title="${p.name}">${p.name}</div>
          <div class="card-meta">
            ${p.w && p.h ? html`${p.w}&times;${p.h}px<br/>` : nothing}
            ${p.modified ? fmtDate(p.modified) : nothing}
          </div>
        </div>
        <button class="card-del" title="${p.name} 삭제"
          @click=${(e) => this._delete(e, p.name)}>
          ${icon("trash", 13)}
        </button>
      </div>
    `;
  }

  _newCardTemplate() {
    return html`
      <div class="card-new" @click=${() => { this._creating = true; }}>
        ${icon("plus", 20)}
        <span class="card-new-label">새 프로젝트</span>
      </div>
    `;
  }

  _formTemplate() {
    return html`
      <div class="card-form">
        <label>
          프로젝트 이름
          <input
            type="text" placeholder="my-design"
            .value=${this._newName}
            @input=${(e) => { this._newName = e.target.value; }}
            @keydown=${(e) => {
              if (e.key === "Enter") this._create();
              if (e.key === "Escape") this._cancelCreate();
            }}
            autofocus
          />
        </label>
        <div class="row">
          <label>
            폭 (px)
            <input type="number" min="1" max="16384" .value=${String(this._newW)}
              @change=${(e) => { this._newW = +e.target.value; }} />
          </label>
          <label>
            높이 (px)
            <input type="number" min="1" max="16384" .value=${String(this._newH)}
              @change=${(e) => { this._newH = +e.target.value; }} />
          </label>
        </div>
        <div class="actions">
          <button class="btn-primary" ?disabled=${this._saving || !this._newName.trim()}
            @click=${() => this._create()}>
            ${this._saving ? "생성 중..." : "만들기"}
          </button>
          <button class="btn-cancel" @click=${() => this._cancelCreate()}>취소</button>
        </div>
      </div>
    `;
  }
}

customElements.define("dx-dashboard", DxDashboard);
