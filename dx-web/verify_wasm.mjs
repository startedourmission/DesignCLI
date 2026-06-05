import { readFile } from "node:fs/promises";
import init, { Editor } from "./src/wasm/pkg/dcli_wasm.js";

const wasmBytes = await readFile(new URL("./src/wasm/pkg/dcli_wasm_bg.wasm", import.meta.url));
await init({ module_or_path: wasmBytes });

const ed = new Editor(64, 48, "u8");
console.log("doc_info:", ed.doc_info());

const actions = [
  { op: "add_paint_layer", name: "bg", source: { from: "fill", rgba: [40, 60, 90, 255] } },
  { op: "add_paint_layer", name: "art", source: { from: "shapes", items: [
      { shape: "ellipse", cx: 32, cy: 24, rx: 16, ry: 12, rgba: [255, 200, 60, 255] },
      { shape: "rect", x: 4, y: 4, w: 10, h: 10, rgba: [255, 0, 0, 200] }
  ]}, bind: "art" },
  { op: "set_blend", id: { bind: "art" }, mode: "multiply" }
];
const res = JSON.parse(ed.apply_actions(JSON.stringify(actions)));
console.log("apply ok:", res.ok, "applied:", res.applied, "bindings:", JSON.stringify(res.bindings));
console.log("layers:", ed.layers());

const rgba = ed.composite_rgba();
console.log("composite_rgba length:", rgba.length, "(expect", 64*48*4, ")");
const ci = (24*64 + 32) * 4;
console.log("center pixel RGBA:", [rgba[ci], rgba[ci+1], rgba[ci+2], rgba[ci+3]]);

console.log("can_undo:", ed.can_undo());
console.log("undo:", ed.undo(), "→ layers:", JSON.parse(ed.layers()).layers.length);
console.log("redo:", ed.redo(), "→ layers:", JSON.parse(ed.layers()).layers.length);

const pkg = ed.to_dxpkg();
const ed2 = Editor.from_dxpkg(pkg);
console.log("dxpkg roundtrip: bytes", pkg.length, "→ reopened layers", JSON.parse(ed2.layers()).layers.length);

const png = ed.export_png();
console.log("png bytes:", png.length, "magic:", [...png.slice(0,4)]);
console.log("\nOK wasm Editor 전 기능 동작");
