// 합성 비용 벤치 — 카드뉴스형 문서(1254², 레이어 30개)에서 viewport 합성/바운드 스캔 시간.
import { readFile } from "node:fs/promises";
import init, { Editor } from "./src/wasm/pkg/dcli_wasm.js";

const wasmBytes = await readFile(new URL("./src/wasm/pkg/dcli_wasm_bg.wasm", import.meta.url));
await init({ module_or_path: wasmBytes });

const J = JSON.stringify;
const ed = new Editor(1254, 1254, "u8");
const acts = [{ op: "add_paint_layer", name: "bg", source: { from: "fill", rgba: [245, 241, 232, 255] } }];
let bindSeq = 0;
const withMeta = (item, extra = {}) => {
  const bind = `b${bindSeq++}`;
  acts.push({ op: "add_paint_layer", name: item.shape, source: { from: "shapes", items: [item] }, bind });
  acts.push({ op: "set_props", id: { bind }, patch: { meta: J(
    item.shape === "text"
      ? { type: "text", x: item.x, y: item.y, text: item.text, size: item.size, rgba: item.rgba }
      : { type: "shape", shape: item.shape, item, fill: item.rgba, rgba: item.rgba, stroke: null, strokeWidth: 0, ...extra }
  ) } });
};
for (let i = 0; i < 12; i++) {
  withMeta({ shape: "rounded_rect", x: 60 + i * 90, y: 80 + (i % 4) * 260, w: 320, h: 200, radius: 24, rgba: [40 + i * 10, 90, 200 - i * 5, 255] });
}
for (let i = 0; i < 10; i++) {
  withMeta({ shape: "text", x: 100, y: 90 + i * 110, text: `카드뉴스 헤드라인 ${i} — Speed Test`, size: 44, rgba: [24, 28, 32, 255] });
}
for (let i = 0; i < 7; i++) {
  withMeta({ shape: "ellipse", cx: 200 + i * 140, cy: 1000, rx: 70, ry: 70, rgba: [220, 120 + i * 15, 60, 200] });
}
const r = JSON.parse(ed.apply_actions(J(acts)));
console.log("레이어 30개 생성 ok:", r.ok);

const time = (label, fn, n = 5) => {
  fn(); // warmup
  const t0 = performance.now();
  for (let i = 0; i < n; i++) fn();
  console.log(`${label}: ${((performance.now() - t0) / n).toFixed(1)}ms`);
};

// 종전(거대 viewport: scene 전체 ≈ 1510²) vs 신규 패드(보이는 영역+25%)
time("composite 1510x1510 (구 viewport, 2.3MP)", () => ed.composite_region_rgba(-128, -128, 1510, 1510));
time("composite 2000x1700 (멀티프레임 구 viewport, 3.4MP)", () => ed.composite_region_rgba(-128, -128, 2000, 1700));
time("composite 1000x750  (신 패드, 화면 1600x900 z=1 가시영역 클립)", () => ed.composite_region_rgba(100, 100, 1000, 750));
time("composite 480x360   (z=2 확대 시 가시영역)", () => ed.composite_region_rgba(300, 300, 480, 360));

// 편집 1회당 종전 비용: 전 레이어 layer_bounds 알파 스캔(캐시 무효화 시)
const ids = JSON.parse(ed.layers()).layers.map((l) => l.id);
time("layer_bounds × 30 (종전: apply마다 전체 재스캔)", () => { for (const id of ids) ed.layer_bounds(id); });
time("hit_test 1회", () => ed.hit_test(600, 600), 20);

// 화면 공간 합성(신형) — 출력 = 화면 + 패드(1856×1056 ≈ 2MP), 장면 크기 무관.
if (typeof ed.composite_view_rgba === "function") {
  console.log("\n── composite_view (화면 공간 + 벡터 재래스터) ──");
  time("view s=0.5 (fit급 줌아웃, 2MP 출력)", () => ed.composite_view_rgba(-100, -100, 0.5, 1856, 1056));
  time("view s=1.0 (디바이스 100%, 정수 시프트)", () => ed.composite_view_rgba(0, 0, 1.0, 1856, 1056));
  time("view s=2.0 (CSS 100%@레티나)", () => ed.composite_view_rgba(100, 100, 2.0, 1856, 1056));
  time("view s=4.0 (확대 — 벡터 재래스터 선명)", () => ed.composite_view_rgba(300, 300, 4.0, 1856, 1056));
}
console.log("\n(bounds는 (id,surface,size) 키로 영구 캐시 — apply당 0회 재스캔)");
