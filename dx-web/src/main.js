// 진입점 — wasm 초기화 → Editor 생성 → App/Renderer 배선 → 컴포넌트 마운트.

import init, { Editor } from "./wasm/pkg/dcli_wasm.js";
import { App, Renderer } from "./app.js";
import "./components.js";

async function main() {
  await init(); // wasm 로드.

  const editor = new Editor(800, 600, "u8");
  // Renderer는 캔버스가 dx-canvas firstUpdated에서 주입됨(임시 캔버스로 시작).
  const renderer = new Renderer(editor, document.createElement("canvas"));
  const app = new App(editor, renderer);

  const shell = document.querySelector("app-shell");
  shell.app = app;

  // 저장(.dxpkg) / export(PNG) 핸들러.
  shell.addEventListener("save-dxpkg", () => download(editor.to_dxpkg(), "untitled.dxpkg", "application/octet-stream"));
  shell.addEventListener("export-png", () => download(editor.export_png(), "export.png", "image/png"));

  // 초기 1프레임.
  app._notify();
  window.__dx = { editor, app }; // 디버그/검증용.
}

function download(bytes, name, type) {
  const blob = new Blob([bytes], { type });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url; a.download = name; a.click();
  URL.revokeObjectURL(url);
}

main().catch((e) => {
  document.body.innerHTML = `<pre style="padding:20px;color:#f88">초기화 실패:\n${e}</pre>`;
  console.error(e);
});
