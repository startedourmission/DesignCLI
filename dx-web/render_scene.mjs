// 시각 검수 산출물 — 표준 장면을 3개 배율로 렌더해 PNG로 남긴다.
// 루프 에이전트는 /tmp/dcli-scene-*.png 를 Read(이미지)로 직접 보고
// 계단현상·위치 어긋남·블렌딩 깨짐을 판정한다.
import { readFile, writeFile } from "node:fs/promises";
import init, { Editor } from "./src/wasm/pkg/dcli_wasm.js";

const wasmBytes = await readFile(new URL("./src/wasm/pkg/dcli_wasm_bg.wasm", import.meta.url));
await init({ module_or_path: wasmBytes });
const J = JSON.stringify;

const ed = new Editor(800, 600, "u8");
let bindSeq = 0;
const acts = [];
const add = (item, meta) => {
  const bind = `b${bindSeq++}`;
  acts.push({ op: "add_paint_layer", name: item.shape ?? "fill", source: item.shape ? { from: "shapes", items: [item] } : item, bind });
  if (meta) acts.push({ op: "set_props", id: { bind }, patch: { meta: J(meta) } });
};
const shapeMeta = (item) => ({ type: "shape", shape: item.shape, item, fill: item.rgba, rgba: item.rgba, stroke: null, strokeWidth: 0 });
const bg = { shape: "rect", x: 0, y: 0, w: 800, h: 600, rgba: [245, 241, 232, 255] };
add(bg, shapeMeta(bg));
const card = { shape: "rounded_rect", x: 60, y: 60, w: 420, h: 300, radius: 28, rgba: [255, 252, 245, 255] };
add(card, shapeMeta(card));
const circle = { shape: "ellipse", cx: 600, cy: 200, rx: 110, ry: 110, rgba: [220, 120, 60, 230] };
add(circle, shapeMeta(circle));
const line = { shape: "line", x0: 80, y0: 480, x1: 720, y1: 430, width: 6, rgba: [40, 60, 90, 255] };
add(line, shapeMeta(line));
const title = { shape: "text", x: 100, y: 110, text: "디자인 검수 Scene — Aa한글123", size: 40, rgba: [24, 28, 32, 255] };
add(title, { type: "text", x: title.x, y: title.y, text: title.text, size: title.size, rgba: title.rgba });
// ── 신기능 검수: 그림자 / 그라데이션 / 채움없음 / 텍스트 배경 ──
const shadowCard = { shape: "rounded_rect", x: 90, y: 200, w: 180, h: 100, radius: 18, rgba: [255, 255, 255, 255] };
{
  const meta = { type: "shape", shape: "rounded_rect", item: shadowCard, fill: shadowCard.rgba, rgba: shadowCard.rgba, stroke: null, strokeWidth: 0, shadow: { dx: 0, dy: 10, blur: 26, rgba: [10, 14, 20, 120] } };
  const items = [
    { shape: "shadow", x: 90, y: 210, w: 180, h: 100, radius: 18, feather: 26, rgba: [10, 14, 20, 120] },
    shadowCard,
  ];
  const bind = `b${bindSeq++}`;
  acts.push({ op: "add_paint_layer", name: "shadow-card", source: { from: "shapes", items }, bind });
  acts.push({ op: "set_props", id: { bind }, patch: { meta: J(meta) } });
}
const gradRect = { shape: "rounded_rect", x: 320, y: 200, w: 180, h: 100, radius: 18, rgba: [13, 153, 255, 255],
  gradient: { x0: 0, y0: 0, x1: 1, y1: 1, radial: false, stops: [{ at: 0, rgba: [255, 120, 80, 255] }, { at: 1, rgba: [90, 70, 220, 255] }] } };
add(gradRect, { type: "shape", shape: "rounded_rect", item: gradRect, fill: gradRect.rgba, rgba: gradRect.rgba, stroke: null, strokeWidth: 0 });
// 채움 없음(테두리만)
{
  const base = { shape: "rounded_rect", x: 545, y: 200, w: 160, h: 100, radius: 18, rgba: [0, 0, 0, 0] };
  const meta = { type: "shape", shape: "rounded_rect", item: base, fill: base.rgba, rgba: base.rgba, stroke: [28, 92, 89, 255], strokeWidth: 5, noFill: true };
  const items = [{ shape: "stroke_rounded_rect", x: base.x, y: base.y, w: base.w, h: base.h, radius: 18, width: 5, rgba: [28, 92, 89, 255] }];
  const bind = `b${bindSeq++}`;
  acts.push({ op: "add_paint_layer", name: "nofill", source: { from: "shapes", items }, bind });
  acts.push({ op: "set_props", id: { bind }, patch: { meta: J(meta) } });
}
// 텍스트 배경(하이라이트)
{
  const tmeta = { type: "text", x: 110, y: 360, text: "배경 텍스트 BG", size: 34, rgba: [16, 22, 31, 255], bg: { rgba: [255, 213, 95, 255] } };
  const bind = `b${bindSeq++}`;
  // 실제 setTextBg 플로우와 동일: 엔진 measure로 배경 박스를 만들어 표면에도 굽는다.
  const [tw, th] = ed.measure_text(tmeta.text, tmeta.size);
  const px = tmeta.size * 0.35, py = tmeta.size * 0.22;
  acts.push({ op: "add_paint_layer", name: "text-bg", source: { from: "shapes", items: [
    { shape: "rounded_rect", x: tmeta.x - px, y: tmeta.y - py, w: tw + px * 2, h: th + py * 2, radius: tmeta.size * 0.18, rgba: tmeta.bg.rgba },
    { shape: "text", x: tmeta.x, y: tmeta.y, text: tmeta.text, size: tmeta.size, rgba: tmeta.rgba }] }, bind });
  acts.push({ op: "set_props", id: { bind }, patch: { meta: J(tmeta) } });
}
const r = JSON.parse(ed.apply_actions(J(acts)));
if (!r.ok) { console.error("장면 생성 실패:", J(r.issues)); process.exit(1); }

const shots = [
  ["fit",  -40, -40, 0.5, 440, 340],   // 줌아웃: 축소 AA 검수(계단·모아레)
  ["100",    0,   0, 1.0, 800, 600],   // 디바이스 1:1: 비트 경로 검수
  ["400",   80,  90, 4.0, 800, 600],   // 확대: 벡터 재래스터 검수(계단 없음, 텍스트 선명)
];
for (const [name, vx, vy, s, w, h] of shots) {
  const png = ed.composite_view_png(vx, vy, s, w, h);
  const path = `/tmp/dcli-scene-${name}.png`;
  await writeFile(path, png);
  console.log(`렌더: ${path} (${w}x${h}, s=${s})`);
}
console.log("시각 산출물 생성 완료 — 에이전트는 위 PNG를 직접 열어 검수할 것");
