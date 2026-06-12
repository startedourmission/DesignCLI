#!/usr/bin/env bash
# 루프 엔지니어링 게이트 — 에이전트/CI가 한 번에 돌리는 전체 검증.
# 사용: bash scripts/check.sh [--fast]
#   --fast: 테스트만(벤치·시각 산출물 생략)
# 종료코드 0 = 전부 통과. 실패 지점은 마지막 출력 참조.
set -euo pipefail
cd "$(dirname "$0")/.."
FAST="${1:-}"

step() { echo; echo "━━━ $1 ━━━"; }

step "1/7 cargo test --workspace (골든·패리티 포함)"
cargo test --workspace --quiet 2>&1 | tail -3

step "2/7 wasm 빌드 (simd128)"
bash dx-web/scripts/build-wasm.sh > /dev/null
echo "ok: dx-web/src/wasm/pkg/"

step "3/7 JS 문법"
node --check dx-web/src/app.js
node --check dx-web/src/components.js
node --check dx-web/src/bridge.js
node --check dx-web/src/live.js
echo "ok"

step "4/7 좌표·offset 회귀 (verify_fixes)"
(cd dx-web && node verify_fixes.mjs | tail -1)

if [ "$FAST" = "--fast" ]; then
  echo; echo "✔ fast 게이트 통과 (벤치·시각·브라우저 스모크 생략)"; exit 0
fi

step "5/7 렌더 벤치 (회귀 감시 — 수치는 추세 비교용)"
(cd dx-web && node bench_composite.mjs | grep -E "view|ok:")

step "6/7 시각 산출물 (/tmp/dcli-scene-*.png — 에이전트가 Read로 직접 검수)"
(cd dx-web && node render_scene.mjs)

step "7/7 브라우저 스모크 (헤드리스 크롬 — 실제 Renderer/ImageData/캔버스 경로)"
# Node 게이트의 사각지대를 막는다: ImageData 브랜드, putImageData 더티렉트,
# drawImage 'copy' 스크롤 시프트는 브라우저에서만 검증 가능(과거 캔버스 블랭크 회귀).
CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
[ -x "$CHROME" ] || CHROME="$(command -v google-chrome || command -v chromium || true)"
if [ -x "$CHROME" ]; then
  SRV_LOG="$(mktemp)"
  # -u 필수: 리다이렉트 시 stdout 블록버퍼링으로 포트 라인이 안 나온다.
  python3 -u -m http.server 0 --bind 127.0.0.1 --directory dx-web > "$SRV_LOG" 2>&1 &
  SRV_PID=$!
  trap 'kill "$SRV_PID" 2>/dev/null || true' EXIT
  PORT=""
  for _ in $(seq 1 50); do
    PORT="$(sed -n 's/.*port \([0-9][0-9]*\).*/\1/p' "$SRV_LOG" | head -1)"
    [ -n "$PORT" ] && break
    sleep 0.1
  done
  SMOKE="$("$CHROME" --headless=new --disable-gpu --enable-logging=stderr --v=0 \
    --virtual-time-budget=15000 "http://127.0.0.1:$PORT/smoke.html" 2>&1 \
    | grep -o 'SMOKE \(PASS\|FAIL\)[^"]*' | head -1 || true)"
  kill "$SRV_PID" 2>/dev/null || true
  echo "${SMOKE:-SMOKE FAIL: 콘솔 출력 없음}"
  case "$SMOKE" in "SMOKE PASS"*) ;; *) exit 1 ;; esac
else
  echo "skip: Chrome/Chromium 없음 — 브라우저 전용 경로는 이 단계만 잡는다"
fi

echo; echo "✔ 전체 게이트 통과. 데몬 반영이 필요하면: cargo build -p dcli-daemon && 데몬 재시작"
