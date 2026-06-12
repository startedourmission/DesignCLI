// AppController — Editor(wasm) 핸들이 유일 진실원. 모든 쓰기는 여기를 통과한다.
// 패널은 editor.layers() 읽기 전용, 쓰기는 apply()로만(이중 상태관리 금지).

import * as B from "./bridge.js";

export class Renderer {
  constructor(editor, canvas) {
    this.editor = editor;
    this._dirty = false;
    this._raf = 0;
    this._timer = 0;
    this.excludeId = null; // 텍스트 인라인 편집 중 화면에서만 제외할 레이어(문서 무오염).
    this.viewport = { x: 0, y: 0, w: editor.width(), h: editor.height() };
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
  setViewport(x, y, w, h) {
    const next = { x: Math.floor(x), y: Math.floor(y), w: Math.max(1, Math.ceil(w)), h: Math.max(1, Math.ceil(h)) };
    const old = this.viewport;
    this.viewport = next;
    if (this.canvas.width !== next.w) this.canvas.width = next.w;
    if (this.canvas.height !== next.h) this.canvas.height = next.h;
    if (!old || old.x !== next.x || old.y !== next.y || old.w !== next.w || old.h !== next.h) this.markDirty();
  }

  /** 신형 wasm이면 화면 공간 합성을 쓴다(보이는 픽셀만 — 장면 크기와 무관). */
  hasViewComposite() {
    return typeof this.editor.composite_view_rgba === "function";
  }

  /** 화면(뷰) 합성 설정. (x,y)=버퍼 (0,0)의 월드 좌표, zoom=문서px→CSSpx,
   *  renderScale=CSSpx→버퍼px(보통 devicePixelRatio — 레티나 선명도). */
  setView(x, y, zoom, cssW, cssH, renderScale) {
    const w = Math.max(1, Math.round(cssW * renderScale));
    const h = Math.max(1, Math.round(cssH * renderScale));
    // 월드 원점을 **디바이스 정수 격자**로 스냅: 팬 델타가 항상 정수 디바이스 px이 되어
    // 보존 프레임이 스크롤 경로(행 memmove + 노출 스트립만 재합성)를 타고, 블릿 위상이
    // 고정돼 서브픽셀 리샘플 블러도 없다. (s=1 정수 시프트 비트 경로도 이 스냅에 포함.)
    const s = zoom * renderScale;
    const qx = Math.round(x * s) / s;
    const qy = Math.round(y * s) / s;
    const next = { x: qx, y: qy, zoom, cssW, cssH, renderScale, w, h };
    const o = this.view;
    this.view = next;
    if (this.canvas.width !== w) this.canvas.width = w;
    if (this.canvas.height !== h) this.canvas.height = h;
    if (!o || o.x !== next.x || o.y !== next.y || o.zoom !== next.zoom
      || o.w !== next.w || o.h !== next.h || o.renderScale !== next.renderScale) this.markDirty();
  }

  resize() {
    if (this.view) {
      // 뷰 모드: 월드 좌표 뷰는 문서 교체와 무관 — 재합성만.
      this.markDirty();
      return;
    }
    this.setViewport(0, 0, this.editor.width(), this.editor.height());
    this.markDirty();
  }
  markDirty() {
    clearTimeout(this._timer);
    this._timer = 0;
    this._dirty = true;
    if (!this._raf) this._raf = requestAnimationFrame(() => this._frame());
  }
  markDirtySoon(delay = 70) {
    this._dirty = true;
    if (this._raf || this._timer) return;
    this._timer = setTimeout(() => {
      this._timer = 0;
      this.markDirty();
    }, delay);
  }
  _frame() {
    this._raf = 0;
    if (!this._dirty) return;
    this._dirty = false;
    if (this.view && this.hasViewComposite()) {
      const v = this.view;
      const s = v.zoom * v.renderScale;
      // 보존 프레임 경로: wasm이 바뀐 픽셀만 다시 만들고 어디가 바뀌었는지 알려준다.
      // 팬 = 캔버스 자체 시프트 + 노출 스트립 업로드, 편집 = 손상 rect만 업로드.
      if (typeof this.editor.render_frame === "function") {
        const ex = this.excludeId != null ? this.excludeId : -1;
        const d = this.editor.render_frame(v.x, v.y, s, v.w, v.h, ex);
        const mode = d[0];
        if (mode === 0) return; // 변화 없음 — 업로드 생략.
        // 제로카피 뷰 — 이 뒤로 putImageData까지 wasm 호출 금지(메모리 성장 시 detach).
        const px = this.editor.frame_pixels();
        if (px.length !== v.w * v.h * 4) return;
        const img = new ImageData(px, v.w, v.h);
        if (mode === 1) {
          this.ctx.putImageData(img, 0, 0);
          return;
        }
        const dx = d[1], dy = d[2], n = d[3];
        if (dx || dy) {
          // 원점이 (dx,dy) 디바이스px 이동 → 기존 픽셀은 (−dx,−dy)로 시프트.
          // 'copy'로 그려 시프트 결과만 남긴다(source-over면 반투명 픽셀이 이중 블렌드).
          const op = this.ctx.globalCompositeOperation;
          this.ctx.globalCompositeOperation = "copy";
          this.ctx.drawImage(this.canvas, -dx, -dy);
          this.ctx.globalCompositeOperation = op;
        }
        for (let i = 0; i < n; i++) {
          const o = 4 + i * 4;
          this.ctx.putImageData(img, 0, 0, d[o], d[o + 1], d[o + 2], d[o + 3]);
        }
        return;
      }
      // 구형 wasm 폴백: 전체 프레임 복사본.
      const canExclude = typeof this.editor.composite_view_rgba_excluding === "function";
      const buf = this.excludeId != null && canExclude
        ? this.editor.composite_view_rgba_excluding(this.excludeId, v.x, v.y, s, v.w, v.h)
        : this.editor.composite_view_rgba(v.x, v.y, s, v.w, v.h);
      this.ctx.putImageData(new ImageData(buf, v.w, v.h), 0, 0);
      return;
    }
    const { x, y, w, h } = this.viewport;
    const canExclude = typeof this.editor.composite_region_rgba_excluding === "function";
    const buf = this.excludeId != null && canExclude
      ? this.editor.composite_region_rgba_excluding(this.excludeId, x, y, w, h)
      : this.editor.composite_region_rgba(x, y, w, h);
    const img = new ImageData(buf, w, h);
    this.ctx.putImageData(img, 0, 0);
  }
}

export class App extends EventTarget {
  constructor(editor, renderer) {
    super();
    this.editor = editor;
    this.renderer = renderer;
    this.live = null; // 라이브 모드면 LiveLink. 설정 시 쓰기가 데몬을 경유.
    this.selectedIds = []; // 선택툴이 고른 레이어 node id 배열(빈 배열=선택 없음). [0]이 primary.
    this.selectedFrameId = null;
    this.clipboardIds = []; // 앱 내부 클립보드 — Cmd+C로 기억한 레이어 id들(문서 내 복제용).
    this._cache = {};
    // 편집을 넘어 살아남는 캐시(문서 교체 시에만 비움) — 핫패스 비용 제거.
    this._metaCache = new Map(); // meta 문자열 → 파싱 결과(동일 문자열은 스냅샷 재생성을 넘어 재사용).
    this._boundsCache = new Map(); // `${id}:${surface}:${w}x${h}` → src bounds(표면 픽셀에만 의존).
  }

  /** meta JSON 파싱(캐시). hover/overlay/패널이 같은 문자열을 반복 파싱하지 않게 한다
   *  (브러시 meta는 점 배열로 수 KB~MB — 포인터무브마다 파싱하면 프레임 드랍). */
  metaOf(l) {
    const s = l?.meta;
    if (!s) return null;
    let v = this._metaCache.get(s);
    if (v === undefined) {
      if (this._metaCache.size > 512) this._metaCache.clear();
      try { v = JSON.parse(s); } catch { v = null; }
      this._metaCache.set(s, v);
    }
    return v;
  }

  /** id → 레이어 dict (캐시 세대당 1회 구축). */
  layerById(id) {
    const m = this._cache.layerById ??= new Map(this.layers().map((l) => [l.id, l]));
    return m.get(id) ?? null;
  }

  _invalidateCache() {
    this._cache = {};
    if (this._warmHandle) {
      const cancel = window.cancelIdleCallback ?? clearTimeout;
      cancel(this._warmHandle);
      this._warmHandle = 0;
    }
  }

  _scheduleGeometryWarmup() {
    if (this._warmHandle) return;
    const schedule = window.requestIdleCallback ?? ((fn) => setTimeout(() => fn({ timeRemaining: () => 8 }), 120));
    this._warmHandle = schedule(() => {
      this._warmHandle = 0;
      try {
        for (const l of this.layers()) this.layerBounds(l.id);
        this.frames();
      } catch {
        // Geometry warmup is opportunistic; normal lazy reads still handle failures.
      }
    }, { timeout: 700 });
  }

  _isOffsetOnly(actions) {
    return Array.isArray(actions) && actions.length > 0 && actions.every((a) => {
      if (a?.op !== "set_props") return false;
      const keys = Object.keys(a.patch ?? {});
      return keys.length === 1 && keys[0] === "offset";
    });
  }

  /** Frame 목록(무한 작업영역의 export 단위). 구버전 wasm이면 빈 배열. */
  frames() {
    if (typeof this.editor.frames !== "function") return [];
    if (this._cache.frames) return this._cache.frames;
    try {
      this._cache.frames = JSON.parse(this.editor.frames()).frames ?? [];
    } catch {
      this._cache.frames = [];
    }
    return this._cache.frames;
  }

  /** Frame 추가(자동 id). */
  addFrame(name, x, y, w, h) {
    const frames = this.frames();
    const id = frames.reduce((m, f) => Math.max(m, f.id + 1), 0);
    this.apply([B.setFrames([...frames, { id, name, x: x | 0, y: y | 0, w: w | 0, h: h | 0 }])]);
    this.selectFrame(id);
  }

  /** Frame 제거. */
  removeFrame(id) {
    this.apply([B.setFrames(this.frames().filter((f) => f.id !== id))]);
    if (this.selectedFrameId === id) this.selectedFrameId = null;
  }

  /** Frame 속성 부분 변경. */
  updateFrame(id, patch) {
    const frames = this.frames();
    const next = frames.map((f) => f.id === id ? { ...f, ...patch } : f);
    this.apply([B.setFrames(next)]);
  }

  selectFrame(id) {
    this.selectedFrameId = id == null ? null : id;
    this.selectedIds = [];
    this._notify();
  }

  getSelectedFrame() {
    if (this.selectedFrameId == null) return null;
    return this.frames().find((f) => f.id === this.selectedFrameId) ?? null;
  }

  /** 레이어 1개 PNG export(피그마 per-layer Export) — wasm 즉석 인코딩. */
  exportLayerPng(l) {
    if (typeof this.editor.export_layer_png !== "function") return false;
    try {
      const png = this.editor.export_layer_png(l.id);
      const url = URL.createObjectURL(new Blob([png], { type: "image/png" }));
      const a = document.createElement("a");
      a.href = url;
      a.download = `${(l.name || `layer-${l.id}`).replace(/[\\/:*?"<>|]/g, "_")}.png`;
      a.click();
      URL.revokeObjectURL(url);
      return true;
    } catch (e) {
      console.error("[export] 레이어 export 실패:", e);
      return false;
    }
  }

  /** Frame 단위 PNG export — 라이브면 데몬 URL, 로컬이면 wasm으로 즉석 인코딩. */
  exportFrame(f) {
    if (this.live) {
      const a = document.createElement("a");
      a.href = `/doc/${this.live.docId}/export.png?frame=${encodeURIComponent(f.name)}`;
      a.download = `${f.name}.png`;
      a.click();
      return;
    }
    const png = this.editor.export_region_png(f.x, f.y, f.w, f.h);
    const url = URL.createObjectURL(new Blob([png], { type: "image/png" }));
    const a = document.createElement("a");
    a.href = url; a.download = `${f.name}.png`; a.click();
    URL.revokeObjectURL(url);
  }

  /** 선택 레이어들을 그룹으로(Cmd+G). 2개 미만이면 무시. */
  groupSelected() {
    if (this.selectedIds.length < 2) return;
    this.apply([B.groupLayers([...this.selectedIds])]);
    this.select(null);
  }

  _metaOf(l) {
    return this.metaOf(l);
  }

  /** dispatch.rs materialize()의 표면 origin(min corner) JS 미러.
   *  엔진은 shapes 소스를 콘텐츠 크기 표면으로 굽고 node.offset = floor(min corner)로 둔다.
   *  재래스터 시 offset을 보정하려면 이 origin이 필요한데, min corner는 글리프 측정 없이
   *  순수 수식이라(텍스트 포함) JS에서 동일하게 계산할 수 있다. (f32 미러: Math.fround) */
  _itemsOrigin(items) {
    let mx = Infinity, my = Infinity;
    for (const it of items ?? []) {
      let x0, y0;
      switch (it?.shape) {
        case "rect": case "rounded_rect":
          if (!(it.w > 0 && it.h > 0)) continue;
          x0 = it.x - 1; y0 = it.y - 1; break;
        case "stroke_rect": case "stroke_rounded_rect": {
          if (!(it.w > 0 && it.h > 0 && it.width > 0)) continue;
          const m = Math.max(it.width, 1);
          x0 = it.x - m; y0 = it.y - m; break;
        }
        case "ellipse":
          if (!(it.rx > 0 && it.ry > 0)) continue;
          x0 = it.cx - it.rx - 1; y0 = it.cy - it.ry - 1; break;
        case "stroke_ellipse": {
          if (!(it.rx > 0 && it.ry > 0 && it.width > 0)) continue;
          const m = Math.max(it.width, 1);
          x0 = it.cx - it.rx - m; y0 = it.cy - it.ry - m; break;
        }
        case "line": {
          if (!(it.width > 0)) continue;
          const m = it.width * 0.5 + 1;
          x0 = Math.min(it.x0, it.x1) - m; y0 = Math.min(it.y0, it.y1) - m; break;
        }
        case "path": {
          if (!(it.width > 0) || (it.points?.length ?? 0) < 4) continue;
          const m = it.width * 0.5 + 1;
          let px = Infinity, py = Infinity;
          for (let i = 0; i + 1 < it.points.length; i += 2) {
            px = Math.min(px, it.points[i]); py = Math.min(py, it.points[i + 1]);
          }
          x0 = px - m; y0 = py - m; break;
        }
        case "text": {
          if (!(it.size > 0) || !it.text) continue;
          const m = Math.max(it.size, 1) * 0.15 + 2;
          x0 = it.x - m; y0 = it.y - m; break;
        }
        case "shadow": {
          if (!(it.w > 0 && it.h > 0)) continue;
          const m = Math.max(it.feather ?? 0, 0) + 1;
          x0 = it.x - m; y0 = it.y - m; break;
        }
        default: continue;
      }
      mx = Math.min(mx, x0); my = Math.min(my, y0);
    }
    if (!Number.isFinite(mx)) return null;
    return [Math.floor(Math.fround(mx)), Math.floor(Math.fround(my))];
  }

  /** 표면이 문서 크기인가(레거시 — origin이 (0,0) 기준). */
  _isDocSizedSurface(l) {
    return Array.isArray(l?.surface_size)
      && l.surface_size[0] === this.editor.width()
      && l.surface_size[1] === this.editor.height();
  }

  /** 재래스터 후에도 현재 화면 위치가 보존되는 offset.
   *  현재 offset은 "마지막 materialize의 origin + 사용자 이동량"이므로, 새 아이템의 origin
   *  변화량만큼 보정한다. 레거시(문서 크기 표면)나 oldItems 미상(local 좌표 fallback)은
   *  origin=(0,0) 기준. 이 보정이 없으면 테두리 추가 시 (width−1)px 드리프트, 레거시
   *  레이어는 좌상단 점프가 발생한다. */
  _rebasedOffset(l, oldItems, newItems) {
    const [ox, oy] = l.offset ?? [0, 0];
    const on = this._itemsOrigin(newItems);
    if (!on) return [ox, oy];
    const oc = (!this._isDocSizedSurface(l) && oldItems?.length)
      ? this._itemsOrigin(oldItems) ?? [0, 0]
      : [0, 0];
    return [ox + on[0] - oc[0], oy + on[1] - oc[1]];
  }

  /** 사용 가능한 글꼴 목록(데몬 스캔). 로컬 모드/실패 시 번들만. */
  async fontList() {
    if (this._fontList) return this._fontList;
    try {
      const r = await fetch("/fonts");
      if (!r.ok) throw new Error(String(r.status));
      const j = await r.json();
      this._fontList = j.fonts ?? ["Pretendard"];
    } catch {
      this._fontList = ["Pretendard"];
    }
    return this._fontList;
  }

  /** 글꼴을 wasm에 등록(1회). 등록되면 벡터 캐시가 비워져 즉시 선명하게 재래스터. */
  async ensureFont(name) {
    if (!name || name === "Pretendard") return true;
    this._loadedFonts ??= new Set();
    if (this._loadedFonts.has(name)) return true;
    if (typeof this.editor.register_font !== "function") return false;
    try {
      const r = await fetch(`/fonts/data?name=${encodeURIComponent(name)}`);
      if (!r.ok) return false;
      const idx = parseInt(r.headers.get("x-face-index") ?? "0", 10) || 0;
      const bytes = new Uint8Array(await r.arrayBuffer());
      this.editor.register_font(name, bytes, idx);
      this._loadedFonts.add(name);
      this.renderer.markDirty();
      return true;
    } catch (e) {
      console.warn("[font] 등록 실패:", name, e);
      return false;
    }
  }

  /** 문서가 쓰는 글꼴들을 미리 등록(열기 직후 — 폴백 글꼴 표시 방지). */
  preloadDocFonts() {
    const names = new Set();
    for (const l of this.layers()) {
      const f = this.metaOf(l)?.font;
      if (f && f !== "Pretendard") names.add(f);
    }
    for (const n of names) this.ensureFont(n);
  }

  /** 텍스트 글꼴 변경(meta.font) — 노드 보존 재래스터. */
  setTextFont(id, font) {
    const l = this.layers().find((v) => v.id === id);
    const meta = this._metaOf(l);
    if (!l || meta?.type !== "text") return false;
    const next = { ...meta };
    if (font && font !== "Pretendard") next.font = font; else delete next.font;
    return this._applyMetaRestyle(l, next);
  }

  /** 엔진과 동일 metric의 텍스트 레이아웃 측정 [w, h] (배경 박스 구성용). */
  measureText(text, size, font) {
    if (typeof this.editor.measure_text === "function") {
      const m = this.editor.measure_text(String(text ?? ""), size, font || undefined);
      return [m[0], m[1]];
    }
    // 구버전 wasm 폴백(근사).
    const lines = String(text ?? "").split("\n");
    const w = Math.max(1, ...lines.map((ln) => ln.length)) * size * 0.55;
    return [w, lines.length * size * 1.25];
  }

  /** meta가 기술하는 전체 아이템 목록 — dispatch::items_from_meta와 동일 규약(단일 소스).
   *  순서: [그림자?, 채움?(noFill이면 생략), 테두리?] / 텍스트: [배경?, 글자].
   *  마지막 materialize와 같은 구성이어야 offset 리베이스가 정확하다. */
  itemsFromMeta(meta) {
    if (!meta) return null;
    if (meta.type === "text") {
      const size = meta.size ?? 32;
      const items = [];
      if (meta.bg?.rgba) {
        const [tw, th] = this.measureText(meta.text, size, meta.font);
        const px = meta.bg.padX ?? size * 0.35;
        const py = meta.bg.padY ?? size * 0.22;
        const bg = B.roundedRect(
          (meta.x ?? 0) - px, (meta.y ?? 0) - py,
          tw + px * 2, th + py * 2,
          meta.bg.radius ?? size * 0.18, meta.bg.rgba,
        );
        if (meta.bg.gradient) bg.gradient = meta.bg.gradient;
        items.push(bg);
      }
      const t = B.text(meta.x ?? 0, meta.y ?? 0, meta.text ?? "", size, meta.rgba ?? [0, 0, 0, 255]);
      if (meta.font) t.font = meta.font;
      items.push(t);
      return items;
    }
    if ((meta.type === "shape" || meta.type === "brush") && meta.item) {
      const it = meta.item;
      const items = [];
      // 그림자 — 채움 지오메트리에서 유도(맨 아래).
      const g = it.shape === "rect" ? { x: it.x, y: it.y, w: it.w, h: it.h, r: 0 }
        : it.shape === "rounded_rect" ? { x: it.x, y: it.y, w: it.w, h: it.h, r: it.radius ?? 0 }
        : it.shape === "ellipse" ? { x: it.cx - it.rx, y: it.cy - it.ry, w: it.rx * 2, h: it.ry * 2, r: Math.min(it.rx, it.ry) }
        : null;
      if (meta.shadow?.rgba && g && g.w > 0 && g.h > 0) {
        items.push({
          shape: "shadow",
          x: g.x + (meta.shadow.dx ?? 0), y: g.y + (meta.shadow.dy ?? 6),
          w: g.w, h: g.h, radius: g.r,
          feather: Math.max(0.5, meta.shadow.blur ?? 16),
          rgba: meta.shadow.rgba,
        });
      }
      if (!meta.noFill) items.push(it);
      const sw = Math.max(0, Number(meta.strokeWidth) || 0);
      if (sw > 0 && meta.stroke && (meta.stroke[3] ?? 255) > 0) {
        const strokeItem = this._strokeShapeItem(it, meta.stroke, sw);
        if (strokeItem) items.push(strokeItem);
      }
      return items.length ? items : null;
    }
    return null;
  }

  /** 현재 meta가 기술하는 아이템 목록(마지막 materialize에 쓰인 것과 동일해야 함). */
  _currentItemsOf(l, meta) {
    return this.itemsFromMeta(meta);
  }

  /** meta 패치 기반 재스타일 공통 경로 — 아이템 재구성 + 위치 보존 리베이스 + 교체. */
  _applyMetaRestyle(l, nextMeta, name) {
    const items = this.itemsFromMeta(nextMeta);
    if (!items) return false;
    const offset = this._rebasedOffset(l, this.itemsFromMeta(this.metaOf(l)) ?? undefined, items);
    return !!this._replacePaintLayer(l, B.shapes(items), nextMeta, name ?? l.name, { offset });
  }

  _shapeWithRgba(item, rgba) {
    return item ? { ...item, rgba } : null;
  }

  _shapeKindOf(meta, l) {
    return String(meta?.shape ?? meta?.item?.shape ?? l?.name ?? "").toLowerCase();
  }

  _fallbackShapeItem(l, meta, rgba) {
    const b = this.layerBounds(l.id);
    if (!b) return null;
    const kind = this._shapeKindOf(meta, l);
    const [x, y, w, h] = b;
    if (w <= 0 || h <= 0) return null;
    if (kind.includes("stroke-ellipse")) return B.strokeEllipse(x + w / 2, y + h / 2, w / 2, h / 2, meta.width ?? 4, rgba);
    if (kind.includes("ellipse")) return B.ellipse(x + w / 2, y + h / 2, w / 2, h / 2, rgba);
    if (kind.includes("stroke-rect")) return B.strokeRect(x, y, w, h, meta.width ?? 4, rgba);
    if (kind.includes("rounded")) return B.roundedRect(x, y, w, h, meta.radius ?? Math.min(w, h) / 6, rgba);
    if (kind.includes("rect") || meta?.type === "shape") return B.rect(x, y, w, h, rgba);
    return null;
  }

  _strokeShapeItem(fillItem, rgba, width) {
    const w = Math.max(0, Number(width) || 0);
    if (!fillItem || w <= 0 || !rgba || (rgba[3] ?? 255) <= 0) return null;
    switch (fillItem.shape) {
      case "rect":
        return B.strokeRect(fillItem.x, fillItem.y, fillItem.w, fillItem.h, w, rgba);
      case "ellipse":
        return B.strokeEllipse(fillItem.cx, fillItem.cy, fillItem.rx, fillItem.ry, w, rgba);
      case "rounded_rect":
        return B.strokeRoundedRect(fillItem.x, fillItem.y, fillItem.w, fillItem.h, fillItem.radius, w, rgba);
      default:
        return null;
    }
  }

  /** 레이어의 픽셀 소스를 교체한다(재스타일/재래스터).
   *  replace_paint_source라 노드 id·그룹 소속·z순서·선택·blend·visible이 전부 보존된다
   *  (예전 delete+add 방식은 그룹 안 레이어에서 "노드 없음"으로 거부됐다).
   *  overrides.offset: 배열=명시값, null=엔진 origin 그대로(아이템 좌표 = 월드 좌표일 때),
   *  미지정=l.offset 유지. overrides.scale: scale 굽기(bake) 시 [1,1] 전달. */
  _replacePaintLayer(l, source, meta, name = l.name, overrides = {}) {
    const patch = {
      name: name || l.name || "layer",
      meta: JSON.stringify(meta),
      scale: overrides.scale ?? l.scale ?? [1, 1],
      rotation: l.rotation ?? 0,
    };
    if (overrides.offset !== null) patch.offset = overrides.offset ?? l.offset ?? [0, 0];
    const res = this.apply([
      B.replacePaintSource(l.id, source),
      B.setProps(l.id, patch),
    ]);
    if (res?.ok === false) console.error("재스타일 실패:", res.issues);
    return res;
  }

  /** shape meta를 보장한다 — 레거시(item 없음)는 bounds에서 item을 복원해 마이그레이션. */
  _ensureShapeMeta(l, meta) {
    if (meta?.item) return meta;
    const rgba = meta?.fill ?? meta?.rgba ?? [13, 153, 255, 255];
    const item = this._fallbackShapeItem(l, meta ?? {}, rgba);
    if (!item) return null;
    return { type: "shape", shape: meta?.shape ?? item.shape, ...meta, item };
  }

  setLayerColor(id, rgba) {
    const l = this.layers().find((v) => v.id === id);
    const meta = this._metaOf(l);
    if (!l || !meta) return false;
    if (meta.type === "text") {
      const next = { ...meta, rgba };
      return this._applyMetaRestyle(l, next, String(next.text ?? l.name).split("\n")[0].slice(0, 20) || l.name);
    }
    if (meta.type === "shape" || meta.type === "brush") {
      const m = this._ensureShapeMeta(l, meta);
      if (!m) return false;
      // 단색 지정은 그라데이션을 해제한다(Figma 동작).
      const item = { ...m.item, rgba };
      delete item.gradient;
      return this._applyMetaRestyle(l, { ...m, item, fill: rgba, rgba, noFill: false });
    }
    return false;
  }

  /** 채움 스타일 — {kind:"solid",rgba} | {kind:"gradient",gradient} | {kind:"none"} */
  setShapeFill(id, spec) {
    const l = this.layers().find((v) => v.id === id);
    const m = this._ensureShapeMeta(l, this._metaOf(l));
    if (!l || !m || m.type !== "shape") return false;
    if (spec.kind === "none") {
      return this._applyMetaRestyle(l, { ...m, noFill: true });
    }
    if (spec.kind === "gradient") {
      const item = { ...m.item, gradient: spec.gradient };
      return this._applyMetaRestyle(l, { ...m, item, noFill: false });
    }
    const item = { ...m.item, rgba: spec.rgba };
    delete item.gradient;
    return this._applyMetaRestyle(l, { ...m, item, fill: spec.rgba, rgba: spec.rgba, noFill: false });
  }

  setShapeStroke(id, stroke, strokeWidth) {
    const l = this.layers().find((v) => v.id === id);
    const m = this._ensureShapeMeta(l, this._metaOf(l));
    if (!l || !m || m.type !== "shape") return false;
    const nextStroke = stroke && (stroke[3] ?? 255) > 0 ? stroke : null;
    const nextWidth = Math.max(0, Math.round(Number(strokeWidth) || 0));
    return this._applyMetaRestyle(l, { ...m, stroke: nextStroke, strokeWidth: nextStroke ? nextWidth : 0 });
  }

  setShapeRadius(id, radius) {
    const l = this.layers().find((v) => v.id === id);
    const m = this._ensureShapeMeta(l, this._metaOf(l));
    if (!l || !m || m.type !== "shape") return false;
    const r = Math.max(0, Math.round(Number(radius) || 0));
    const it = m.item;
    let item = it;
    if (it.shape === "rect" || it.shape === "rounded_rect") {
      item = r > 0
        ? { ...it, shape: "rounded_rect", radius: r }
        : (() => { const v = { ...it, shape: "rect" }; delete v.radius; return v; })();
    }
    return this._applyMetaRestyle(l, { ...m, item, radius: r });
  }

  /** 그림자 — {dx, dy, blur, rgba} 또는 null(제거). */
  setShapeShadow(id, shadow) {
    const l = this.layers().find((v) => v.id === id);
    const m = this._ensureShapeMeta(l, this._metaOf(l));
    if (!l || !m || m.type !== "shape") return false;
    const next = { ...m };
    if (shadow) next.shadow = shadow; else delete next.shadow;
    return this._applyMetaRestyle(l, next);
  }

  /** 텍스트 배경 — {rgba, padX?, padY?, radius?, gradient?} 또는 null(제거). */
  setTextBg(id, bg) {
    const l = this.layers().find((v) => v.id === id);
    const meta = this._metaOf(l);
    if (!l || meta?.type !== "text") return false;
    const next = { ...meta };
    if (bg) next.bg = bg; else delete next.bg;
    return this._applyMetaRestyle(l, next);
  }

  setTextSize(id, size) {
    const l = this.layers().find((v) => v.id === id);
    const meta = this._metaOf(l);
    if (!l || meta?.type !== "text") return false;
    const next = { ...meta, size: Math.max(6, Math.min(400, Math.round(size || meta.size || 32))) };
    return this._applyMetaRestyle(l, next, String(next.text ?? l.name).split("\n")[0].slice(0, 20) || l.name);
  }

  /** 도형/브러시 리사이즈를 벡터 재래스터로 굽는다(비파괴 scale → 지오메트리 확정).
   *  scale 보간(블러/형태 붕괴) 대신 새 크기로 다시 그린다. 회전 레이어는 표면 중심이
   *  바뀌면 위치가 틀어지므로 제외(기존 scale 방식 유지). 성공 시 true. */
  bakeShapeScale(id, scale, offset) {
    const l = this.layers().find((v) => v.id === id);
    const meta = this._metaOf(l);
    const bakeable = (meta?.type === "shape" || meta?.type === "brush") && meta?.item
      && (l?.rotation ?? 0) === 0;
    if (!bakeable) return false;
    const prov = { ...l, scale: scale ?? l.scale ?? [1, 1], offset: offset ?? l.offset ?? [0, 0] };
    const t = this.xformOf(prov);
    // item 좌표 → src 좌표(− origin) → 월드 좌표. 결과 아이템은 월드 좌표가 된다.
    const oc = this._isDocSizedSurface(l)
      ? [0, 0]
      : this._itemsOrigin(this._currentItemsOf(l, meta)) ?? [0, 0];
    const W = (px, py) => t.fwd(px - oc[0], py - oc[1]);
    const [asx, asy] = [Math.abs(prov.scale[0] || 1), Math.abs(prov.scale[1] || 1)];
    const it = meta.item;
    let baked = null;
    if (it.shape === "rect" || it.shape === "rounded_rect") {
      const a = W(it.x, it.y), b = W(it.x + it.w, it.y + it.h);
      const x = Math.min(a.x, b.x), y = Math.min(a.y, b.y);
      baked = { ...it, x, y, w: Math.abs(b.x - a.x), h: Math.abs(b.y - a.y) };
    } else if (it.shape === "ellipse") {
      const c = W(it.cx, it.cy);
      baked = { ...it, cx: c.x, cy: c.y, rx: it.rx * asx, ry: it.ry * asy };
    } else if (it.shape === "line") {
      const a = W(it.x0, it.y0), b = W(it.x1, it.y1);
      baked = { ...it, x0: a.x, y0: a.y, x1: b.x, y1: b.y, width: Math.max(0.5, it.width * (asx + asy) / 2) };
    } else if (it.shape === "path") {
      const pts = [];
      for (let i = 0; i + 1 < (it.points?.length ?? 0); i += 2) {
        const p = W(it.points[i], it.points[i + 1]);
        pts.push(p.x, p.y);
      }
      baked = { ...it, points: pts, width: Math.max(0.5, it.width * (asx + asy) / 2) };
    }
    if (!baked) return false;
    // 굽힌 아이템은 월드 좌표 — 엔진 origin을 그대로 쓰고(offset 생략) scale은 1로 리셋.
    // 그림자/테두리/noFill/그라데이션은 meta에서 itemsFromMeta가 재구성한다.
    const nextMeta = { ...meta, item: baked };
    const items = this.itemsFromMeta(nextMeta);
    if (!items) return false;
    return !!this._replacePaintLayer(l, B.shapes(items), nextMeta, l.name, { offset: null, scale: [1, 1] });
  }

  /** 선택이 그룹이면 해제(Cmd+Shift+G). */
  ungroupSelected() {
    const l = this.getSelected();
    if (!l || l.kind !== "group") return;
    this.apply([B.ungroup(l.id)]);
    this.select(null);
  }

  /** Cmd+C — 선택 id들을 내부 클립보드에 기억(즉시 복제 아님). */
  copy(ids) {
    if (!ids?.length) return;
    this.clipboardIds = [...ids];
  }

  /** Cmd+V — 기억한 레이어 중 아직 살아있는 것만 복제(없어진 id는 무시). */
  paste() {
    if (!this.clipboardIds.length) return;
    const alive = new Set(this.layers().map((l) => l.id));
    this.duplicateMany(this.clipboardIds.filter((id) => alive.has(id)));
  }

  /** 단일 선택(교체). null이면 해제. changed로 캔버스·패널 동기. */
  select(id) {
    this.selectedFrameId = null;
    this.selectedIds = id == null ? [] : [id];
    this._notify();
  }

  /** Shift+클릭 토글 — 이미 선택이면 제거, 아니면 추가. */
  toggleSelect(id) {
    if (id == null) return;
    this.selectedFrameId = null;
    this.selectedIds = this.selectedIds.includes(id)
      ? this.selectedIds.filter((v) => v !== id)
      : [...this.selectedIds, id];
    this._notify();
  }

  /** 다중 선택 교체(마퀴 등). 중복 제거. */
  selectMany(ids) {
    this.selectedFrameId = null;
    this.selectedIds = [...new Set(ids ?? [])];
    this._notify();
  }

  /** 기존 단일 선택 호환 — 첫 번째 선택 id(없으면 null). */
  get selectedId() {
    return this.selectedIds[0] ?? null;
  }

  /** 선택된 레이어 객체(첫 번째, top-to-bottom 목록에서 조회). 없으면 null. */
  getSelected() {
    if (this.selectedId == null) return null;
    return this.layers().find((l) => l.id === this.selectedId) || null;
  }

  /** 선택된 모든 레이어 객체(top-to-bottom 순). */
  selectedLayers() {
    const set = new Set(this.selectedIds);
    return this.layers().filter((l) => set.has(l.id));
  }

  /** 캔버스 좌표 hit-test → 최상위 레이어 id(없으면 null). */
  hitTest(x, y) {
    const textId = this.textBoxHitTest(x, y);
    if (textId != null) return textId;
    const id = this.editor.hit_test(Math.floor(x), Math.floor(y));
    return id < 0 ? null : id;
  }

  textBoxHitTest(x, y) {
    // 텍스트 레이어 목록은 캐시 세대당 1회만 추림(포인터무브 핫패스).
    const txts = this._cache.textLayers ??= this.layers().filter((l) => this.metaOf(l)?.type === "text");
    for (const l of txts) {
      const meta = this.metaOf(l);
      const box = this.textBoxBounds(l, meta);
      if (!box) continue;
      const p = this.xformOf(l).inv(x, y);
      if (p.x >= box.x && p.x <= box.x + box.w && p.y >= box.y && p.y <= box.y + box.h) return l.id;
    }
    return null;
  }

  textBoxBounds(l, meta) {
    const size = meta.size ?? 32;
    const padX = Math.max(8, size * 0.22);
    const padY = Math.max(5, size * 0.16);
    const raster = this.layerBounds(l.id);
    if (raster) {
      return {
        x: raster[0] - padX,
        y: raster[1] - padY,
        w: raster[2] + padX * 2,
        h: raster[3] + padY * 2,
      };
    }
    const lines = String(meta.text ?? "").split("\n");
    const ctx = this._measureCtx ??= document.createElement("canvas").getContext("2d");
    ctx.font = `${size}px Inter, system-ui, sans-serif`;
    const width = Math.max(1, ...lines.map((line) => ctx.measureText(line || " ").width));
    const lineHeight = size * 1.25;
    return {
      x: (meta.x ?? 0) - padX,
      y: (meta.y ?? 0) - padY,
      w: width + padX * 2,
      h: Math.max(lineHeight, lines.length * lineHeight) + padY * 2,
    };
  }

  /** 레이어 불투명 src 바운드 [x,y,w,h](표면 로컬 좌표 — offset 미포함) 또는 null.
   *  src 바운드는 표면 픽셀에만 의존하므로 (node, surface, 크기)가 같으면 편집을 넘어
   *  재사용한다 — 없으면 apply마다 모든 레이어를 wasm 알파 스캔(O(W×H)×N)해 매우 느리다.
   *  (표면 in-place 페인팅 기능이 생기면 revision 키로 교체할 것 — 현재는 교체 방식만.) */
  layerBounds(id) {
    const l = this.layerById(id);
    const key = l?.surface != null
      ? `${id}:${l.surface}:${l.surface_size?.[0]}x${l.surface_size?.[1]}`
      : null; // 그룹은 자식 파생이라 캐시 불가.
    if (key) {
      const hit = this._boundsCache.get(key);
      if (hit !== undefined) return hit;
    }
    const bounds = JSON.parse(this.editor.layer_bounds(id));
    if (key) {
      if (this._boundsCache.size > 1024) this._boundsCache.clear();
      this._boundsCache.set(key, bounds);
    }
    return bounds;
  }

  /** 레이어 복제(Cmd+D). 데몬/로컬 공통 — DuplicateLayer Action. */
  duplicate(id) {
    if (id == null) return;
    this.apply([B.duplicateLayer(id)]);
  }

  /** 여러 레이어 복제 — 하나의 apply 배치로. */
  duplicateMany(ids) {
    if (!ids?.length) return;
    this.apply(ids.map((id) => B.duplicateLayer(id)));
  }

  /** 여러 레이어 삭제 — 하나의 apply 배치로. */
  deleteMany(ids) {
    if (!ids?.length) return;
    const res = this.apply(ids.map((id) => B.deleteLayer(id)));
    if (!res?.deferred) this.cleanupSingletonGroups();
  }

  cleanupSingletonGroups() {
    const acts = [];
    const visit = (l) => {
      for (const child of l.children ?? []) visit(child);
      if (l.kind !== "group") return;
      const n = l.children?.length ?? 0;
      if (n === 1) acts.push(B.ungroup(l.id));
      else if (n === 0) acts.push(B.deleteLayer(l.id));
    };
    for (const root of this.layerTree()) visit(root);
    if (acts.length) this.apply(acts);
  }

  /** 선택 레이어 offset 상대이동(화살표 nudge). */
  nudge(id, dx, dy) {
    const l = this.layers().find((v) => v.id === id);
    if (!l) return;
    const [ox, oy] = l.offset ?? [0, 0];
    this.apply([B.setOffset(id, [ox + dx, oy + dy])]);
  }

  /** 여러 레이어 offset 상대이동 — 하나의 apply 배치로. */
  nudgeMany(ids, dx, dy) {
    if (!ids?.length) return;
    const layers = this.layers();
    const acts = [];
    for (const id of ids) {
      const l = layers.find((v) => v.id === id);
      if (!l) continue;
      const [ox, oy] = l.offset ?? [0, 0];
      acts.push(B.setOffset(id, [ox + dx, oy + dy]));
    }
    if (acts.length) this.apply(acts);
  }

  /** 레이어의 표시 트랜스폼(엔진과 동일 수학: 표면 중심 기준 scale→rotate→offset). */
  xformOf(l) {
    const [W, H] = Array.isArray(l.surface_size)
      ? l.surface_size
      : [this.editor.width(), this.editor.height()];
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

  /** 스케일/회전 변경 시 anchor(src 좌표)의 캔버스 위치가 고정되도록 보정된 offset.
   *  T(p) = R(θ)·(S⊙(p−c)) + c + off 에서:
   *  off' = off + R(θ0)(S0⊙a) − R(θ1)(S1⊙a),  a = anchor − c.
   *  리사이즈는 반대쪽 핸들, 회전은 도형 중심을 anchor로 주면 Figma처럼 제자리 변형. */
  computeAnchoredOffset(l, newScale, newRotation, anchorSrc) {
    const t = this.xformOf(l);
    const [s0x, s0y] = l.scale ?? [1, 1];
    const [ox, oy] = l.offset ?? [0, 0];
    const [s1x, s1y] = newScale ?? l.scale ?? [1, 1];
    const r1 = (((newRotation ?? l.rotation ?? 0) * Math.PI) / 180);
    const ax = anchorSrc.x - t.c.x, ay = anchorSrc.y - t.c.y;
    const rot = (rad, x, y) => ({ x: Math.cos(rad) * x - Math.sin(rad) * y, y: Math.sin(rad) * x + Math.cos(rad) * y });
    const r0 = (((l.rotation ?? 0) * Math.PI) / 180);
    const p0 = rot(r0, s0x * ax, s0y * ay);
    const p1 = rot(r1, s1x * ax, s1y * ay);
    return [Math.round(ox + p0.x - p1.x), Math.round(oy + p0.y - p1.y)];
  }

  /** 선택 레이어 좌우/상하 뒤집기 — scale 부호 반전, 도형(bbox) 중심 고정. axis: "x"|"y".
   *  하나의 apply 배치로 처리(undo 한 번에 묶임). */
  flipMany(ids, axis) {
    if (!ids?.length) return;
    const layers = this.layers();
    const acts = [];
    for (const id of ids) {
      const l = layers.find((v) => v.id === id);
      if (!l) continue;
      const b = this.layerBounds(id);
      if (!b) continue;
      const [sx, sy] = l.scale ?? [1, 1];
      const ns = axis === "x" ? [-sx, sy] : [sx, -sy];
      // anchor = 도형(불투명 bbox) 중심 — 뒤집어도 제자리에 머문다.
      const anchor = { x: b[0] + b[2] / 2, y: b[1] + b[3] / 2 };
      acts.push(B.setProps(id, { scale: ns, offset: this.computeAnchoredOffset(l, ns, null, anchor) }));
    }
    if (acts.length) this.apply(acts);
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

  sceneBounds() {
    if (this._cache.sceneBounds) return this._cache.sceneBounds;
    const boxes = [{ x: 0, y: 0, w: this.editor.width(), h: this.editor.height() }];
    for (const l of this.layers()) {
      if (!l.visible) continue;
      const box = this.displayAABB(l);
      if (box) boxes.push(box);
    }
    for (const f of this.frames()) boxes.push({ x: f.x, y: f.y, w: f.w, h: f.h });
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const b of boxes) {
      x0 = Math.min(x0, b.x);
      y0 = Math.min(y0, b.y);
      x1 = Math.max(x1, b.x + b.w);
      y1 = Math.max(y1, b.y + b.h);
    }
    const pad = 128;
    this._cache.sceneBounds = {
      x: Math.floor(x0 - pad),
      y: Math.floor(y0 - pad),
      w: Math.max(1, Math.ceil(x1 - x0 + pad * 2)),
      h: Math.max(1, Math.ceil(y1 - y0 + pad * 2)),
    };
    return this._cache.sceneBounds;
  }

  /** 문서 기준 정렬 setOffset Action 생성(계산 불가면 null). */
  _alignAction(l, mode) {
    const box = this.displayAABB(l);
    if (!box) return null;
    const W = this.editor.width(), H = this.editor.height();
    const [ox, oy] = l.offset ?? [0, 0];
    let dx = 0, dy = 0;
    if (mode === "left") dx = -box.x;
    else if (mode === "center-h") dx = (W - box.w) / 2 - box.x;
    else if (mode === "right") dx = W - box.w - box.x;
    else if (mode === "top") dy = -box.y;
    else if (mode === "center-v") dy = (H - box.h) / 2 - box.y;
    else if (mode === "bottom") dy = H - box.h - box.y;
    return B.setOffset(l.id, [Math.round(ox + dx), Math.round(oy + dy)]);
  }

  /** 선택 레이어를 문서 기준 정렬: left|center-h|right|top|center-v|bottom */
  align(id, mode) {
    const l = this.layers().find((v) => v.id === id);
    const a = l ? this._alignAction(l, mode) : null;
    if (a) this.apply([a]);
  }

  /** 여러 레이어를 문서 기준 정렬 — 하나의 apply 배치로(각자 정렬). */
  alignMany(ids, mode) {
    if (!ids?.length) return;
    const layers = this.layers();
    const acts = ids
      .map((id) => {
        const l = layers.find((v) => v.id === id);
        return l ? this._alignAction(l, mode) : null;
      })
      .filter(Boolean);
    if (acts.length) this.apply(acts);
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

  /** 여러 레이어를 한 칸 위로 — 하나의 apply 배치. 위에서부터 처리해 상대 순서 유지,
   *  맨 위에 붙은 선택 블록은 그대로 둔다(천장). 액션 순서대로 시뮬레이션해 인덱스 산출. */
  raiseMany(ids) {
    if (!ids?.length) return;
    const order = this.orderBottomToTop();
    const set = new Set(ids);
    const acts = [];
    let ceiling = order.length; // 이 인덱스 이상으로는 못 올라감.
    for (let i = order.length - 1; i >= 0; i--) {
      if (!set.has(order[i])) continue;
      if (i + 1 >= ceiling) { ceiling = i; continue; }
      const id = order[i];
      order.splice(i, 1);
      order.splice(i + 1, 0, id);
      acts.push(B.moveLayer(id, i + 1));
    }
    if (acts.length) this.apply(acts);
  }

  /** 여러 레이어를 한 칸 아래로 — 하나의 apply 배치. 아래에서부터 처리(바닥 블록 유지). */
  lowerMany(ids) {
    if (!ids?.length) return;
    const order = this.orderBottomToTop();
    const set = new Set(ids);
    const acts = [];
    let floor = -1; // 이 인덱스 이하로는 못 내려감.
    for (let i = 0; i < order.length; i++) {
      if (!set.has(order[i])) continue;
      if (i - 1 <= floor) { floor = i; continue; }
      const id = order[i];
      order.splice(i, 1);
      order.splice(i - 1, 0, id);
      acts.push(B.moveLayer(id, i - 1));
    }
    if (acts.length) this.apply(acts);
  }

  /** 라이브 모드 진입(쓰기 funnel을 데몬으로 전환). live.js가 호출. */
  setLive(link) {
    this.live = link;
  }

  /** 라이브 snapshot으로 editor 교체 + 렌더러 재배선 + 첫 프레임. */
  replaceEditor(editor) {
    this.editor = editor;
    this.renderer.editor = editor;
    this._invalidateCache();
    this._boundsCache.clear(); // 문서가 통째로 바뀜 — (id,surface) 키 신뢰 불가.
    this._metaCache.clear();
    this.renderer.resize();
    this._notify();
    this._scheduleGeometryWarmup();
    this.preloadDocFonts();
  }

  /** 원격(데몬 broadcast) 적용 후 화면·패널 갱신. */
  afterRemoteApply() {
    this._invalidateCache();
    this.renderer.markDirty();
    this._notify({ layoutChanged: true });
    this._scheduleGeometryWarmup();
  }

  afterOptimisticAck() {
    this._scheduleGeometryWarmup();
  }

  /** Action 배열을 적용한다(단일 쓰기 funnel).
   *  로컬/라이브 모두 먼저 로컬 editor에 적용한다. 라이브 echo는 LiveLink가 seq만 소비한다. */
  apply(actions) {
    const encoded = JSON.stringify(actions);
    const offsetOnly = this._isOffsetOnly(actions);
    const res = JSON.parse(this.editor.apply_actions(encoded));
    if (!res.ok) {
      console.error("apply 실패:", res.issues);
      this._invalidateCache();
      this._notify();
      return res;
    }
    this._invalidateCache();
    if (offsetOnly) {
      this.renderer.markDirtySoon();
      this._notify({ geometryOnly: true });
      this._scheduleGeometryWarmup();
    } else {
      this.renderer.markDirty();
      this._notify({ layoutChanged: true });
      this._scheduleGeometryWarmup();
    }
    if (this.live) {
      this.live.sendApply(actions, encoded);
      return { ...res, optimistic: true, geometryOnly: offsetOnly };
    }
    return { ...res, geometryOnly: offsetOnly };
  }

  undo() {
    if (this.live) {
      this.live.sendUndo();
      return;
    }
    if (this.editor.undo()) {
      this._invalidateCache();
      this.renderer.markDirty();
      this._notify({ layoutChanged: true });
      this._scheduleGeometryWarmup();
    }
  }
  redo() {
    if (this.live) {
      this.live.sendRedo();
      return;
    }
    if (this.editor.redo()) {
      this._invalidateCache();
      this.renderer.markDirty();
      this._notify({ layoutChanged: true });
      this._scheduleGeometryWarmup();
    }
  }

  /** 레이어 목록(파생 뷰, top-to-bottom). */
  layerTree() {
    if (this._cache.layerTree) return this._cache.layerTree;
    const j = JSON.parse(this.editor.layers());
    this._cache.layerTree = j.layers.slice().reverse(); // bottom-to-top → 표시용 top-to-bottom
    return this._cache.layerTree;
  }
  layers() {
    if (this._cache.layers) return this._cache.layers;
    const out = [];
    const visit = (l) => {
      out.push(l);
      for (const child of l.children ?? []) visit(child);
    };
    for (const root of this.layerTree()) visit(root);
    this._cache.layers = out;
    return this._cache.layers;
  }
  canUndo() { return this.editor.can_undo(); }
  canRedo() { return this.editor.can_redo(); }
  docInfo() { return JSON.parse(this.editor.doc_info()); }

  _notify(detail = null) {
    // 삭제된 레이어 id가 선택에 남지 않도록 정리.
    if (this.selectedIds.length) {
      const alive = new Set(this.layers().map((l) => l.id));
      this.selectedIds = this.selectedIds.filter((id) => alive.has(id));
    }
    this.dispatchEvent(new CustomEvent("changed", { detail }));
  }
}
