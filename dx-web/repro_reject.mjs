// 사용자 콘솔의 "apply 실패/편집 거부" 재현 — 실제 문서의 meta 구조로 UI 편집 배치를 재연.
import { readFile } from "node:fs/promises";
import init, { Editor } from "./src/wasm/pkg/dcli_wasm.js";

const wasmBytes = await readFile(new URL("./src/wasm/pkg/dcli_wasm_bg.wasm", import.meta.url));
await init({ module_or_path: wasmBytes });
const J = JSON.stringify;
const apply = (ed, acts) => JSON.parse(ed.apply_actions(J(acts)));
const layersOf = (ed) => { const out = []; const visit = (l) => { out.push(l); (l.children ?? []).forEach(visit); }; JSON.parse(ed.layers()).layers.forEach(visit); return out; };
const report = (name, r) => console.log(r.ok ? `PASS ${name}` : `FAIL ${name} → ${J(r.issues)}`);

const ed = new Editor(1254, 1254, "u8");

// 실제 문서와 같은 형태의 레이어들 구성.
const panel = { shape: "rounded_rect", x: 86, y: 96, w: 1082, h: 1062, radius: 54, rgba: [255, 255, 255, 226] };
const title = { x: 127, y: 273, text: "CLI 카드뉴스", size: 92, rgba: [18, 27, 35, 255] };
let r = apply(ed, [
  { op: "add_paint_layer", name: "bg", source: { from: "fill", rgba: [240, 238, 230, 255] } },
  { op: "add_paint_layer", name: "main-panel", source: { from: "shapes", items: [panel] }, bind: "p" },
  { op: "set_props", id: { bind: "p" }, patch: { meta: J({ type: "shape", shape: "rounded-rect", item: panel, fill: panel.rgba, rgba: panel.rgba, stroke: null, strokeWidth: 0, radius: 54 }) } },
  { op: "add_paint_layer", name: "title", source: { from: "shapes", items: [{ shape: "text", ...title }] }, bind: "t" },
  { op: "set_props", id: { bind: "t" }, patch: { meta: J({ type: "text", ...title }) } },
]);
report("문서 구성", r);
const ids = layersOf(ed).map((l) => l.id);
const panelId = ids[1], titleId = ids[2];

// 1) 테두리 추가(setShapeStroke 재연: delete → add(fill+stroke) → setProps)
{
  const stroke = { shape: "stroke_rounded_rect", x: panel.x, y: panel.y, w: panel.w, h: panel.h, radius: 54, width: 8, rgba: [20, 24, 28, 255] };
  const idx = JSON.parse(ed.layers()).layers.map((l) => l.id).indexOf(panelId);
  r = apply(ed, [
    { op: "delete_layer", id: { node: panelId } },
    { op: "add_paint_layer", name: "main-panel", source: { from: "shapes", items: [panel, stroke] }, index: idx >= 0 ? idx : undefined, bind: "styled" },
    { op: "set_props", id: { bind: "styled" }, patch: { meta: J({ type: "shape", item: panel, stroke: stroke.rgba, strokeWidth: 8, radius: 54 }), offset: [78, 88], scale: [1, 1], rotation: 0 } },
  ]);
  report("테두리 추가 배치", r);
}

// 2) 텍스트 편집 커밋 재연
{
  const idx = JSON.parse(ed.layers()).layers.map((l) => l.id).indexOf(titleId);
  r = apply(ed, [
    { op: "delete_layer", id: { node: titleId } },
    { op: "add_paint_layer", name: "CLI 카드뉴스!", source: { from: "shapes", items: [{ shape: "text", ...title, text: "CLI 카드뉴스!" }] }, index: idx >= 0 ? idx : undefined, bind: "t" },
    { op: "set_props", id: { bind: "t" }, patch: { meta: J({ type: "text", ...title, text: "CLI 카드뉴스!" }), offset: [111, 250], scale: [1, 1], rotation: 0 } },
  ]);
  report("텍스트 편집 커밋 배치", r);
}

// 3) 그룹 안 레이어 재스타일(index −1 케이스: orderBottomToTop은 루트만 본다)
{
  const a = { shape: "rect", x: 100, y: 100, w: 80, h: 60, rgba: [200, 0, 0, 255] };
  r = apply(ed, [
    { op: "add_paint_layer", name: "ga", source: { from: "shapes", items: [a] }, bind: "ga" },
    { op: "set_props", id: { bind: "ga" }, patch: { meta: J({ type: "shape", shape: "rect", item: a, fill: a.rgba, rgba: a.rgba, stroke: null, strokeWidth: 0 }) } },
    { op: "add_paint_layer", name: "gb", source: { from: "shapes", items: [{ ...a, x: 220 }] }, bind: "gb" },
    { op: "group_layers", ids: [{ bind: "ga" }, { bind: "gb" }], name: "group" },
  ]);
  report("그룹 구성", r);
  const inGroup = layersOf(ed).find((l) => l.name === "ga");
  // 구방식(delete+add)은 그룹 자식에서 node_not_found — 신방식 replace_paint_source 검증.
  r = apply(ed, [
    { op: "replace_paint_source", id: { node: inGroup.id }, source: { from: "shapes", items: [{ ...a, rgba: [0, 200, 0, 255] }] } },
    { op: "set_props", id: { node: inGroup.id }, patch: { meta: inGroup.meta, offset: [99, 99], scale: [1, 1], rotation: 0 } },
  ]);
  report("그룹 내 레이어 재스타일(replace_paint_source)", r);
  const after = layersOf(ed).find((l) => l.id === inGroup.id);
  console.log(after ? `PASS 노드 보존(id=${after.id}, 그룹 소속 유지)` : "FAIL 노드 소멸");
  // undo/redo 동작 확인
  console.log(ed.undo() ? "PASS undo" : "FAIL undo", "/", ed.redo() ? "PASS redo" : "FAIL redo");
}

// 4) 의심: undefined index 직렬화 — JSON.stringify가 index:undefined 키를 떨군다 ✓
//    프레임 업데이트(set_frames) — frames 구조 그대로 재적용
{
  r = apply(ed, [{ op: "set_frames", frames: [{ id: 0, name: "card", x: 0, y: 0, w: 1254, h: 1254 }] }]);
  report("set_frames", r);
  r = apply(ed, [{ op: "set_frames", frames: [{ id: 0, name: "card", x: -5, y: 0, w: 1259, h: 1254 }] }]);
  report("set_frames 음수 좌표", r);
}

// 5) 불투명도/blend/가시성 — 단순 set_props 류
{
  const someId = layersOf(ed)[0].id;
  r = apply(ed, [{ op: "set_props", id: { node: someId }, patch: { opacity: 0.55 } }]);
  report("opacity", r);
  r = apply(ed, [{ op: "set_blend", id: { node: someId }, mode: "multiply" }]);
  report("blend", r);
}
console.log("재현 시도 완료");
