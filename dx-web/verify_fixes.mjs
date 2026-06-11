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
      case "ellipse": x0 = it.cx - it.rx - 1; y0 = it.cy - it.ry - 1; break;
      case "stroke_ellipse": { const m = Math.max(it.width, 1); x0 = it.cx - it.rx - m; y0 = it.cy - it.ry - m; break; }
      case "line": { const m = it.width * 0.5 + 1; x0 = Math.min(it.x0, it.x1) - m; y0 = Math.min(it.y0, it.y1) - m; break; }
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

console.log(fails === 0 ? "\n전체 PASS" : `\n${fails}개 FAIL`);
process.exit(fails ? 1 : 0);
