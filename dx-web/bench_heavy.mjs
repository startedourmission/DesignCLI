// 무거운 표시 경로 벤치 — 그라데이션 배경 + 그림자 카드 + 텍스트(반투명 다층).
//
// 표준 bench_composite와 달리 **감마 블렌드가 지배하는** 장면이다: 반투명 그림자
// 래스터가 화면 대부분을 덮어, 캐시 히트 프레임도 픽셀당 OETF/EOTF 비용을 치른다.
// 2026-06 측정(LUT 블렌드 전): 캐시미스 ~350ms / 캐시히트 ~231ms → 회귀 감시용.
import { readFile } from "node:fs/promises";
import init, { Editor } from "./src/wasm/pkg/dcli_wasm.js";

const wasmBytes = await readFile(new URL("./src/wasm/pkg/dcli_wasm_bg.wasm", import.meta.url));
await init({ module_or_path: wasmBytes });

const J = JSON.stringify;
const ed = new Editor(1600, 1000, "u8");
const acts = [];
let bindSeq = 0;
const addShape = (item, meta) => {
  const bind = `b${bindSeq++}`;
  acts.push({ op: "add_paint_layer", name: item.shape, source: { from: "shapes", items: [item] }, bind });
  acts.push({ op: "set_props", id: { bind }, patch: { meta: J(meta) } });
};

// 1) 그라데이션 배경(문서 전체) — 줌 변경 캐시미스 때 다MP 재래스터.
const bgItem = { shape: "rect", x: 0, y: 0, w: 1600, h: 1000, rgba: [40, 44, 80, 255] };
addShape(bgItem, {
  type: "shape", shape: "rect", item: bgItem, fill: [40, 44, 80, 255], rgba: [40, 44, 80, 255],
  stroke: null, strokeWidth: 0,
  gradient: { x0: 0, y0: 0, x1: 1, y1: 1, radial: false, stops: [[0, [40, 44, 80, 255]], [1, [120, 60, 140, 255]]] },
});

// 2) 그림자 카드 4장 — 반투명 feather 래스터가 캐시히트 프레임의 감마 블렌드 비용.
for (let i = 0; i < 4; i++) {
  const x = 120 + (i % 2) * 720;
  const y = 100 + Math.floor(i / 2) * 440;
  const card = { shape: "rounded_rect", x, y, w: 600, h: 360, radius: 20, rgba: [250, 250, 252, 255] };
  addShape(card, {
    type: "shape", shape: "rounded_rect", item: card, fill: card.rgba, rgba: card.rgba,
    stroke: null, strokeWidth: 0,
    shadow: { dx: 0, dy: 10, blur: 36, rgba: [0, 0, 0, 110] },
  });
}

// 3) 텍스트 10개(AA 가장자리 = 부분 알파 → 감마 블렌드 경로).
for (let i = 0; i < 10; i++) {
  const t = { shape: "text", x: 160 + (i % 2) * 720, y: 150 + Math.floor(i / 2) * 170, text: `헤드라인 ${i} — Heavy Display Bench`, size: 40, rgba: [30, 32, 40, 255] };
  addShape(t, { type: "text", x: t.x, y: t.y, text: t.text, size: t.size, rgba: t.rgba });
}

const r = JSON.parse(ed.apply_actions(J(acts)));
if (!r.ok) { console.error("장면 생성 실패", r); process.exit(1); }
console.log(`장면 생성 ok (그라데이션 bg + 그림자 카드 4 + 텍스트 10)`);

const W = 1856, H = 1056;
const frame = (s, vx = -60, vy = -40) => ed.composite_view_rgba(vx, vy, s, W, H);
const time = (label, fn, n = 5) => {
  const t0 = performance.now();
  for (let i = 0; i < n; i++) fn();
  console.log(`${label}: ${((performance.now() - t0) / n).toFixed(1)}ms`);
};

// 캐시미스: 매번 다른 줌(재래스터 포함). 캐시히트: 같은 줌 반복(팬만).
let z = 0.77;
time("줌 변경(캐시 미스) s≈0.77~", () => { frame(z); z += 0.013; }, 5);
frame(1.3); // warm
time("같은 줌(캐시 히트) s=1.3 팬", () => frame(1.3, -60 + Math.sin(z) * 5, -40), 10);
let z2 = 2.0;
time("줌 변경(캐시 미스) s≈2.x", () => { frame(z2); z2 += 0.017; }, 5);
