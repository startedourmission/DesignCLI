// 수정 검증 — app.js의 offset 리베이스/bake 수학을 그대로 재연해 wasm으로 수치 확인.
import { readFile } from "node:fs/promises";
import init, { Editor } from "./src/wasm/pkg/dcli_wasm.js";

const wasmBytes = await readFile(new URL("./src/wasm/pkg/dcli_wasm_bg.wasm", import.meta.url));
await init({ module_or_path: wasmBytes });

const J = JSON.stringify;
const layersOf = (ed) => JSON.parse(ed.layers()).layers;
const flat = (ls, out = []) => { for (const l of ls) { out.push(l); flat(l.children ?? [], out); } return out; };
const apply = (ed, acts) => JSON.parse(ed.apply_actions(J(acts)));
let fails = 0;
const expect = (name, cond, detail) => {
  console.log(`${cond ? "PASS" : "FAIL"} ${name}${cond ? "" : " — " + detail}`);
  if (!cond) fails++;
};

// app.js _itemsOrigin 미러(검증용 — 동일 수식).
function itemsOrigin(items) {
  let mx = Infinity, my = Infinity;
  for (const it of items ?? []) {
    let x0, y0;
    switch (it?.shape) {
      case "rect": case "rounded_rect": x0 = it.x - 1; y0 = it.y - 1; break;
      case "stroke_rect": case "stroke_rounded_rect": { const m = Math.max(it.width, 1); x0 = it.x - m; y0 = it.y - m; break; }
      case "ellipse": case "polygon": x0 = it.cx - it.rx - 1; y0 = it.cy - it.ry - 1; break;
      case "stroke_ellipse": case "stroke_polygon": { const m = Math.max(it.width, 1); x0 = it.cx - it.rx - m; y0 = it.cy - it.ry - m; break; }
      case "line": { const m = it.width * 0.5 + 1; x0 = Math.min(it.x0, it.x1) - m; y0 = Math.min(it.y0, it.y1) - m; break; }
      case "curve": {
        let px = it.points[0], py = it.points[1], sx = 0, sy = 0;
        for (let i = 2; i + 1 < it.points.length; i += 2) {
          px = Math.min(px, it.points[i]); py = Math.min(py, it.points[i + 1]);
          sx = Math.max(sx, Math.abs(it.points[i] - it.points[i - 2]));
          sy = Math.max(sy, Math.abs(it.points[i + 1] - it.points[i - 1]));
        }
        x0 = px - (it.width * 0.5 + 1 + 0.5 * sx); y0 = py - (it.width * 0.5 + 1 + 0.5 * sy); break;
      }
      case "polygon_path": {
        let px = it.points[0], py = it.points[1];
        for (let i = 2; i + 1 < it.points.length; i += 2) { px = Math.min(px, it.points[i]); py = Math.min(py, it.points[i + 1]); }
        x0 = px - 1; y0 = py - 1; break;
      }
      case "stroke_polygon_path": {
        const m = Math.max(it.width, 1);
        let px = it.points[0], py = it.points[1];
        for (let i = 2; i + 1 < it.points.length; i += 2) { px = Math.min(px, it.points[i]); py = Math.min(py, it.points[i + 1]); }
        x0 = px - m; y0 = py - m; break;
      }
      case "text": { const m = Math.max(it.size, 1) * 0.15 + 2; x0 = it.x - m; y0 = it.y - m; break; }
      default: continue;
    }
    mx = Math.min(mx, x0); my = Math.min(my, y0);
  }
  if (!Number.isFinite(mx)) return null;
  return [Math.floor(Math.fround(mx)), Math.floor(Math.fround(my))];
}
const rebased = (offset, oldItems, newItems, docSized = false) => {
  const on = itemsOrigin(newItems);
  const oc = (!docSized && oldItems?.length) ? itemsOrigin(oldItems) : [0, 0];
  return [offset[0] + on[0] - oc[0], offset[1] + on[1] - oc[1]];
};

// ── 1. 엔진 origin 미러 정확성: 다양한 아이템으로 add 후 node.offset과 비교 ──
{
  const cases = [
    [{ shape: "rect", x: 120, y: 90, w: 100, h: 60, rgba: [200, 0, 0, 255] }],
    [{ shape: "rounded_rect", x: 33.5, y: 21.25, w: 80, h: 40, radius: 9, rgba: [0, 200, 0, 255] }],
    [{ shape: "ellipse", cx: 200, cy: 150, rx: 45.5, ry: 30, rgba: [0, 0, 200, 255] }],
    [{ shape: "stroke_ellipse", cx: 100, cy: 100, rx: 40, ry: 40, width: 7, rgba: [9, 9, 9, 255] }],
    [{ shape: "line", x0: 30.7, y0: 200, x1: 180, y1: 40.2, width: 5, rgba: [1, 2, 3, 255] }],
    [{ shape: "polygon", cx: 150, cy: 120, rx: 60.5, ry: 50, sides: 5, rgba: [255, 0, 100, 255] }],
    [{ shape: "stroke_polygon", cx: 150, cy: 120, rx: 60.5, ry: 50, sides: 6, width: 7, rgba: [9, 9, 9, 255] }],
    [{ shape: "curve", points: [40, 200, 120, 60.5, 220, 180, 300, 90], width: 6, rgba: [20, 40, 200, 255] }],
    [{ shape: "polygon_path", points: [150, 40.25, 245, 110, 187, 153, 90, 222, 55.5, 110], rgba: [255, 180, 0, 255] }],
    [{ shape: "text", x: 100, y: 80, text: "안녕 DX", size: 41, rgba: [10, 20, 30, 255] }],
    [
      { shape: "rect", x: 50, y: 50, w: 60, h: 60, rgba: [9, 9, 9, 255] },
      { shape: "stroke_rect", x: 50, y: 50, w: 60, h: 60, width: 12, rgba: [200, 200, 0, 255] },
    ],
  ];
  for (const items of cases) {
    const ed = new Editor(400, 300, "u8");
    const r = apply(ed, [{ op: "add_paint_layer", name: "x", source: { from: "shapes", items }, bind: "x" }]);
    const l = flat(layersOf(ed)).find((v) => v.id === r.bindings.x.node);
    const mine = itemsOrigin(items);
    expect(`origin 미러 (${items.map((i) => i.shape).join("+")})`,
      l.offset[0] === mine[0] && l.offset[1] === mine[1],
      `engine=${J(l.offset)} mirror=${J(mine)}`);
  }
}

// ── 1b. 점 편집: 정다각형 → 자유 다각형 변환 시 미이동 꼭짓점 월드 위치 보존 ──
// app.js setShapePoints 플로우 미러: points는 item 좌표, rebase가 origin 시프트 보정.
{
  const ed = new Editor(400, 300, "u8");
  const poly = { shape: "polygon", cx: 200, cy: 150, rx: 60, ry: 50, sides: 5, rgba: [120, 80, 220, 255] };
  let r = apply(ed, [{ op: "add_paint_layer", name: "poly", source: { from: "shapes", items: [poly] }, bind: "d" }]);
  const id0 = r.bindings.d.node;
  let l0 = flat(layersOf(ed)).find((l) => l.id === id0);
  // 사용자 이동 +20,+8 → offset = origin + (20,8).
  apply(ed, [{ op: "set_props", id: { node: id0 }, patch: { offset: [l0.offset[0] + 20, l0.offset[1] + 8] } }]);
  l0 = flat(layersOf(ed)).find((l) => l.id === id0);
  // shapePoints 미러: 정다각형 전개(item 좌표 = cx/cy 기준).
  const n = 5, base = [];
  for (let k = 0; k < n; k++) { const a = -Math.PI / 2 + k * 2 * Math.PI / n; base.push(poly.cx + poly.rx * Math.cos(a), poly.cy + poly.ry * Math.sin(a)); }
  // 안 건드린 둘째 꼭짓점(오른쪽 위)의 변경 전 월드 위치.
  const oc0 = itemsOrigin([poly]);
  const v2WorldBefore = [base[2] - oc0[0] + l0.offset[0], base[3] - oc0[1] + l0.offset[1]];
  // 첫 꼭짓점만 위로 당김(item 좌표) → polygon_path로 변환.
  const after = [...base]; after[1] -= 30;
  const newItems = [{ shape: "polygon_path", points: after, rgba: poly.rgba }];
  const off = rebased(l0.offset, [poly], newItems);
  const idx = layersOf(ed).map((l) => l.id).indexOf(id0);
  r = apply(ed, [
    { op: "delete_layer", id: { node: id0 } },
    { op: "add_paint_layer", name: "poly", source: { from: "shapes", items: newItems }, index: idx, bind: "s" },
    { op: "set_props", id: { bind: "s" }, patch: { offset: off, scale: [1, 1], rotation: 0 } },
  ]);
  const id1 = r.bindings.s.node;
  const l1 = flat(layersOf(ed)).find((l) => l.id === id1);
  const oc1 = itemsOrigin(newItems);
  const v2WorldAfter = [after[2] - oc1[0] + l1.offset[0], after[3] - oc1[1] + l1.offset[1]];
  expect("점 편집: polygon→polygon_path 변환 후 미이동 꼭짓점 월드 보존",
    Math.abs(v2WorldAfter[0] - v2WorldBefore[0]) <= 1 && Math.abs(v2WorldAfter[1] - v2WorldBefore[1]) <= 1,
    `before=${J(v2WorldBefore)} after=${J(v2WorldAfter)}`);
}

// ── 2. B8 수정: 테두리 추가 시 fill 위치 보존 ──
{
  const ed = new Editor(400, 300, "u8");
  const item = { shape: "rect", x: 120, y: 90, w: 100, h: 60, rgba: [200, 60, 60, 255] };
  let r = apply(ed, [
    { op: "add_paint_layer", name: "rect", source: { from: "shapes", items: [item] }, bind: "d" },
  ]);
  const id0 = r.bindings.d.node;
  let l0 = flat(layersOf(ed)).find((l) => l.id === id0);
  // 사용자 이동 +37,+13
  apply(ed, [{ op: "set_props", id: { node: id0 }, patch: { offset: [l0.offset[0] + 37, l0.offset[1] + 13] } }]);
  l0 = flat(layersOf(ed)).find((l) => l.id === id0);
  const b0 = JSON.parse(ed.layer_bounds(id0));
  const world0 = [l0.offset[0] + b0[0], l0.offset[1] + b0[1]];

  const strokeItem = { shape: "stroke_rect", x: 120, y: 90, w: 100, h: 60, width: 8, rgba: [20, 24, 28, 255] };
  const newItems = [item, strokeItem];
  const off = rebased(l0.offset, [item], newItems);
  const idx = layersOf(ed).map((l) => l.id).indexOf(id0);
  r = apply(ed, [
    { op: "delete_layer", id: { node: id0 } },
    { op: "add_paint_layer", name: "rect", source: { from: "shapes", items: newItems }, index: idx, bind: "s" },
    { op: "set_props", id: { bind: "s" }, patch: { offset: off, scale: [1, 1], rotation: 0 } },
  ]);
  const id1 = r.bindings.s.node;
  const l1 = flat(layersOf(ed)).find((l) => l.id === id1);
  const b1 = JSON.parse(ed.layer_bounds(id1));
  const world1 = [l1.offset[0] + b1[0], l1.offset[1] + b1[1]];
  expect("B8: stroke 추가 후 fill 월드 위치 보존", world1[0] === world0[0] && world1[1] === world0[1],
    `before=${J(world0)} after=${J(world1)}`);
}

// ── 3. B6 수정: 레거시(문서 크기 표면, offset=0) 텍스트 색상 변경 위치 보존 ──
{
  const ed = new Editor(400, 300, "u8");
  // 레거시 흉내: 문서 크기 fill 표면 + text meta. 실제 글리프 위치는 시뮬레이션이므로
  // "world = item 좌표 + offset" 계약만 검증한다.
  let r = apply(ed, [{ op: "add_paint_layer", name: "legacy", source: { from: "fill", rgba: [0, 0, 0, 0] }, bind: "t" }]);
  const id0 = r.bindings.t.node;
  const l0 = flat(layersOf(ed)).find((l) => l.id === id0);
  const docSized = l0.surface_size[0] === 400 && l0.surface_size[1] === 300;
  const items = [{ shape: "text", x: 200, y: 150, text: "Hi", size: 40, rgba: [255, 0, 0, 255] }];
  const off = rebased(l0.offset ?? [0, 0], items, items, docSized);
  const idx = layersOf(ed).map((l) => l.id).indexOf(id0);
  r = apply(ed, [
    { op: "delete_layer", id: { node: id0 } },
    { op: "add_paint_layer", name: "Hi", source: { from: "shapes", items }, index: idx, bind: "s" },
    { op: "set_props", id: { bind: "s" }, patch: { offset: off, scale: [1, 1], rotation: 0 } },
  ]);
  const id1 = r.bindings.s.node;
  const l1 = flat(layersOf(ed)).find((l) => l.id === id1);
  const b1 = JSON.parse(ed.layer_bounds(id1));
  const worldX = l1.offset[0] + b1[0];
  expect("B6: 레거시 텍스트 편집 후 x≈200 유지(좌상단 점프 없음)", Math.abs(worldX - 200) <= 4,
    `worldX=${worldX} (기대 ≈200, 이전 버그는 ≈10)`);
}

// ── 4. bake: rect 2배 확대 = scale 보간 대신 지오메트리 재래스터 ──
{
  const ed = new Editor(400, 300, "u8");
  const item = { shape: "rect", x: 50, y: 50, w: 40, h: 30, rgba: [10, 200, 50, 255] };
  let r = apply(ed, [{ op: "add_paint_layer", name: "r", source: { from: "shapes", items: [item] }, bind: "d" }]);
  const id0 = r.bindings.d.node;
  const l0 = flat(layersOf(ed)).find((l) => l.id === id0);
  // bake 미러: scale [2,2] anchor=좌상단(50,50) → 기대 지오메트리 (50,50,80,60)
  const oc = itemsOrigin([item]);
  const c = [l0.surface_size[0] / 2, l0.surface_size[1] / 2];
  const fwd = (px, py, s, off2) => [
    (px - oc[0] - c[0]) * s[0] + c[0] + off2[0],
    (py - oc[1] - c[1]) * s[1] + c[1] + off2[1],
  ];
  // computeAnchoredOffset 미러(rot 0): off' = off + S0⊙a − S1⊙a, a = anchor−c (anchor: src 좌표 = item−origin)
  const aSrc = [item.x - oc[0], item.y - oc[1]];
  const a = [aSrc[0] - c[0], aSrc[1] - c[1]];
  const offAnch = [Math.round(l0.offset[0] + a[0] - 2 * a[0]), Math.round(l0.offset[1] + a[1] - 2 * a[1])];
  const p0 = fwd(item.x, item.y, [2, 2], offAnch);
  const p1 = fwd(item.x + item.w, item.y + item.h, [2, 2], offAnch);
  const baked = { ...item, x: Math.min(p0[0], p1[0]), y: Math.min(p0[1], p1[1]), w: Math.abs(p1[0] - p0[0]), h: Math.abs(p1[1] - p0[1]) };
  expect("bake 지오메트리 (50,50,80,60)", baked.x === 50 && baked.y === 50 && baked.w === 80 && baked.h === 60, J(baked));
  const idx = layersOf(ed).map((l) => l.id).indexOf(id0);
  r = apply(ed, [
    { op: "delete_layer", id: { node: id0 } },
    { op: "add_paint_layer", name: "r", source: { from: "shapes", items: [baked] }, index: idx, bind: "s" },
    { op: "set_props", id: { bind: "s" }, patch: { scale: [1, 1], rotation: 0 } },
  ]);
  const id1 = r.bindings.s.node;
  const l1 = flat(layersOf(ed)).find((l) => l.id === id1);
  const b1 = JSON.parse(ed.layer_bounds(id1));
  const world = [l1.offset[0] + b1[0], l1.offset[1] + b1[1], b1[2], b1[3]];
  expect("bake 후 월드 bbox=(50,50,80,60), scale=[1,1]",
    world[0] === 50 && world[1] === 50 && world[2] === 80 && world[3] === 60
    && l1.scale[0] === 1 && l1.scale[1] === 1,
    `world=${J(world)} scale=${J(l1.scale)}`);
}

// ── 5. 보존 프레임: 스크롤 팬·damage 재합성이 신규 풀 합성과 픽셀 동일 ──
// (깨지면 화면에 이음새·잔상이 조용히 생긴다 — render_frame의 핵심 불변식.)
{
  const SCENE = [
    { op: "add_paint_layer", name: "bg", source: { from: "shapes", items: [{ shape: "rect", x: 0, y: 0, w: 400, h: 300, rgba: [240, 235, 226, 255] }] }, bind: "bg" },
    { op: "set_props", id: { bind: "bg" }, patch: { meta: J({ type: "shape", shape: "rect", item: { shape: "rect", x: 0, y: 0, w: 400, h: 300, rgba: [240, 235, 226, 255] }, fill: [240, 235, 226, 255], rgba: [240, 235, 226, 255], stroke: null, strokeWidth: 0 }) } },
    { op: "add_paint_layer", name: "card", source: { from: "shapes", items: [{ shape: "rounded_rect", x: 60, y: 50, w: 180, h: 120, radius: 12, rgba: [255, 255, 255, 255] }] }, bind: "c" },
    { op: "set_props", id: { bind: "c" }, patch: { opacity: 0.9, meta: J({ type: "shape", shape: "rounded_rect", item: { shape: "rounded_rect", x: 60, y: 50, w: 180, h: 120, radius: 12, rgba: [255, 255, 255, 255] }, fill: [255, 255, 255, 255], rgba: [255, 255, 255, 255], stroke: null, strokeWidth: 0, shadow: { dx: 0, dy: 6, blur: 18, rgba: [0, 0, 0, 90] } }) } },
    { op: "add_paint_layer", name: "img", source: { from: "shapes", items: [{ shape: "rect", x: 200, y: 140, w: 120, h: 90, rgba: [80, 120, 200, 220] }] } }, // meta 없음 = 이미지 경로
  ];
  const mk = () => { const e = new Editor(400, 300, "u8"); const r = apply(e, SCENE); if (!r.ok) throw new Error("scene: " + J(r.issues)); return e; };
  const px = (ed) => Uint8ClampedArray.from(ed.frame_pixels()); // 뷰 → 복사
  const s = 1.3, W = 320, H = 240;
  const grid = (v) => Math.round(v * s) / s; // setView 스냅 미러
  const v1 = [grid(10), grid(8)], v0 = [grid(10 - 24 / s), grid(8 - 16 / s)];

  // (a) 팬 스크롤: v0에서 굽고 v1으로 스크롤 == v1 신규 풀 합성.
  const edA = mk();
  edA.render_frame(v0[0], v0[1], s, W, H, -1);
  const dA = edA.render_frame(v1[0], v1[1], s, W, H, -1);
  const fresh1 = mk();
  fresh1.render_frame(v1[0], v1[1], s, W, H, -1);
  expect("보존 프레임: 팬이 스크롤 경로(mode 2)", dA[0] === 2 && (dA[1] !== 0 || dA[2] !== 0), `mode=${dA[0]} d=(${dA[1]},${dA[2]})`);
  {
    const a = px(edA), b = px(fresh1);
    let diff = -1;
    for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) { diff = i; break; }
    expect("보존 프레임: 스크롤 == 신규 풀 합성(픽셀 동일)", diff === -1, `첫 불일치 byte ${diff} (px ${(diff / 4) | 0})`);
  }

  // (b) damage: 카드 이동 후 증분 재합성 == 같은 문서 신규 풀 합성.
  const cardId = flat(layersOf(edA)).find((l) => l.name === "card").id;
  apply(edA, [{ op: "set_props", id: { node: cardId }, patch: { offset: [40, 24] } }]);
  const dB = edA.render_frame(v1[0], v1[1], s, W, H, -1);
  const fresh2 = mk();
  const cardId2 = flat(layersOf(fresh2)).find((l) => l.name === "card").id;
  apply(fresh2, [{ op: "set_props", id: { node: cardId2 }, patch: { offset: [40, 24] } }]);
  fresh2.render_frame(v1[0], v1[1], s, W, H, -1);
  expect("보존 프레임: 편집이 증분 경로(mode 2, rect>0)", dB[0] === 2 && dB[3] > 0, `mode=${dB[0]} n=${dB[3]}`);
  const a = px(edA), b = px(fresh2);
  let diff = -1;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) { diff = i; break; }
  expect("보존 프레임: damage 재합성 == 신규 풀 합성(픽셀 동일)", diff === -1, `첫 불일치 byte ${diff} (px ${(diff / 4) | 0})`);
}

console.log(fails === 0 ? "\n전체 PASS" : `\n${fails}개 FAIL`);
process.exit(fails ? 1 : 0);
