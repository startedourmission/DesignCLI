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
  resize() {
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
    // 매 프레임 새 복사본을 받는다(메모리 detach 무관 — wasm이 소유 복사본 반환).
    // composite_rgba_excluding은 신형 wasm에만 있음 — 구버전 캐시여도 렌더는 살리기(feature-detect).
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
    for (const l of this.layers()) {
      let meta = null;
      try { meta = l.meta ? JSON.parse(l.meta) : null; } catch { /* ignore */ }
      if (meta?.type !== "text") continue;
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

  /** 레이어 불투명 바운드 [x,y,w,h](offset 반영) 또는 null. */
  layerBounds(id) {
    const cache = this._cache.layerBounds ??= new Map();
    if (cache.has(id)) return cache.get(id);
    const bounds = JSON.parse(this.editor.layer_bounds(id));
    cache.set(id, bounds);
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
    this.renderer.resize();
    this._notify();
    this._scheduleGeometryWarmup();
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
