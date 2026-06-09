// 진입점 — wasm 초기화 → Editor 생성 → App/Renderer 배선 → 컴포넌트 마운트.
//
// 모드: URL에 ?doc=<id>가 있으면 라이브 모드(데몬 dx-daemon에 연결, CLI 편집 실시간 반영).
//       없으면 기존 로컬 자족 모드(브라우저 안에서만 편집).

import init, { Editor } from "./wasm/pkg/dcli_wasm.js";
import { App, Renderer } from "./app.js";
import { connectLive } from "./live.js";
import "./components.js";

async function main() {
  await init(); // wasm 로드.

  const docId = new URLSearchParams(location.search).get("doc");

  // 초기 editor: 라이브면 곧 snapshot으로 교체됨(임시 빈 문서로 시작).
  const editor = new Editor(800, 600, "u8");
  // Renderer는 캔버스가 dx-canvas firstUpdated에서 주입됨(임시 캔버스로 시작).
  const renderer = new Renderer(editor, document.createElement("canvas"));
  const app = new App(editor, renderer);

  const shell = document.querySelector("app-shell");
  shell.app = app;

  // 저장(.dxpkg) / export(PNG) — 항상 현재 editor 기준(라이브면 snapshot으로 교체된 것).
  shell.addEventListener("save-dxpkg", () => download(app.editor.to_dxpkg(), "untitled.dxpkg", "application/octet-stream"));
  shell.addEventListener("export-png", () => download(app.editor.export_png(), "export.png", "image/png"));

  if (docId) {
    // 라이브 모드: 데몬 snapshot 로드 + WS 구독. 쓰기는 데몬 경유.
    try {
      await connectLive(app, docId);
      console.log(`[live] 데몬 연결: doc="${docId}"`);
    } catch (e) {
      console.error("[live] 연결 실패, 로컬 모드로:", e);
    }
  }

  // 초기 1프레임.
  app._notify();
  window.__dx = { app, get editor() { return app.editor; } }; // 디버그/검증용.
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
