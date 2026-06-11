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
          // 큰 문서는 snapshot 직렬화·전송이 수 초 — 로딩 오버레이로 상태를 보여준다.
          const loading = showLoading(`"${docId}" 불러오는 중…`);
          try {
            await connectLive(app, docId);
            console.log(`[live] 데몬 연결: doc="${docId}"`);
          } catch (e) {
            console.error("[live] 연결 실패, 로컬 모드로:", e);
          } finally {
            loading.remove();
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

/** 라이브 snapshot 로딩 오버레이 — 150ms 안에 끝나면 표시하지 않는다(플래시 방지). */
function showLoading(msg) {
  const el = document.createElement("div");
  el.style.cssText = [
    "position:fixed", "inset:0", "z-index:9999", "display:flex",
    "align-items:center", "justify-content:center", "pointer-events:none",
    "background:rgba(10,12,14,0.35)", "backdrop-filter:blur(2px)",
    "opacity:0", "transition:opacity 160ms ease",
  ].join(";");
  const box = document.createElement("div");
  box.style.cssText = [
    "display:flex", "align-items:center", "gap:10px", "padding:12px 18px",
    "background:var(--bg-panel, #1d2125)", "color:var(--fg, #e8eaed)",
    "border:1px solid var(--line, #333)", "border-radius:10px",
    "font:500 12.5px Inter, Pretendard, sans-serif",
    "box-shadow:0 14px 38px rgba(0,0,0,0.4)",
  ].join(";");
  const spin = document.createElement("span");
  spin.style.cssText = "width:14px;height:14px;border-radius:50%;border:2px solid var(--accent,#87b9cf);border-top-color:transparent;animation:dxspin 0.8s linear infinite";
  const style = document.createElement("style");
  style.textContent = "@keyframes dxspin{to{transform:rotate(360deg)}}";
  box.append(style, spin, document.createTextNode(msg));
  el.appendChild(box);
  document.body.appendChild(el);
  const t = setTimeout(() => { el.style.opacity = "1"; }, 150);
  return {
    remove() {
      clearTimeout(t);
      el.style.opacity = "0";
      setTimeout(() => el.remove(), 180);
    },
  };
}

function download(bytes, name, type) {
  const blob = new Blob([bytes], { type });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url; a.download = name; a.click();
  URL.revokeObjectURL(url);
}
