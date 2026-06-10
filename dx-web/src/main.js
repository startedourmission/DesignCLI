// 진입점 — URL에 ?doc=<id>가 있으면 에디터 모드, 없으면 대시보드 모드.
//
// 에디터 모드: wasm 초기화 → Editor 생성 → App/Renderer 배선 → app-shell 마운트.
// 대시보드 모드: wasm/라이브 연결 불필요 — <dx-dashboard>만 마운트.

const docId = new URLSearchParams(location.search).get("doc");

if (!docId) {
  // ── 대시보드 모드 ──
  // wasm 초기화 없이 대시보드 컴포넌트만 마운트.
  import("./dashboard.js").then(() => {
    document.body.innerHTML = "";
    const dash = document.createElement("dx-dashboard");
    document.body.appendChild(dash);
  }).catch((e) => {
    document.body.innerHTML = `<pre style="padding:20px;color:#f88">대시보드 초기화 실패:\n${e}</pre>`;
    console.error(e);
  });
} else {
  // ── 에디터 모드 ──
  import("./wasm/pkg/dcli_wasm.js").then(({ default: init, Editor }) => {
    return import("./app.js").then(({ App, Renderer }) => {
      return import("./live.js").then(({ connectLive }) => {
        return import("./components.js").then(async () => {
          await init(); // wasm 로드.

          // 초기 editor: 라이브면 곧 snapshot으로 교체됨(임시 빈 문서로 시작).
          const editor = new Editor(800, 600, "u8");
          // Renderer는 캔버스가 dx-canvas firstUpdated에서 주입됨(임시 캔버스로 시작).
          const renderer = new Renderer(editor, document.createElement("canvas"));
          const app = new App(editor, renderer);

          // app-shell이 없으면 body를 교체해서 마운트.
          let shell = document.querySelector("app-shell");
          if (!shell) {
            document.body.innerHTML = "";
            shell = document.createElement("app-shell");
            document.body.appendChild(shell);
          }
          shell.app = app;

          // 저장(.dxpkg) / export(PNG) — 항상 현재 editor 기준(라이브면 snapshot으로 교체된 것).
          shell.addEventListener("save-dxpkg", () => download(app.editor.to_dxpkg(), "untitled.dxpkg", "application/octet-stream"));
          shell.addEventListener("export-png", () => download(app.editor.export_png(), "export.png", "image/png"));

          // 라이브 모드: 데몬 snapshot 로드 + WS 구독. 쓰기는 데몬 경유.
          try {
            await connectLive(app, docId);
            console.log(`[live] 데몬 연결: doc="${docId}"`);
          } catch (e) {
            console.error("[live] 연결 실패, 로컬 모드로:", e);
          }

          // 초기 1프레임.
          app._notify();
          window.__dx = { app, get editor() { return app.editor; } }; // 디버그/검증용.
        });
      });
    });
  }).catch((e) => {
    document.body.innerHTML = `<pre style="padding:20px;color:#f88">초기화 실패:\n${e}</pre>`;
    console.error(e);
  });
}

function download(bytes, name, type) {
  const blob = new Blob([bytes], { type });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url; a.download = name; a.click();
  URL.revokeObjectURL(url);
}
