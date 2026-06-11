#!/usr/bin/env bash
# 루프 엔지니어링 게이트 — 에이전트/CI가 한 번에 돌리는 전체 검증.
# 사용: bash scripts/check.sh [--fast]
#   --fast: 테스트만(벤치·시각 산출물 생략)
# 종료코드 0 = 전부 통과. 실패 지점은 마지막 출력 참조.
set -euo pipefail
cd "$(dirname "$0")/.."
FAST="${1:-}"

step() { echo; echo "━━━ $1 ━━━"; }

step "1/6 cargo test --workspace (골든·패리티 포함)"
cargo test --workspace --quiet 2>&1 | tail -3

step "2/6 wasm 빌드 (simd128)"
bash dx-web/scripts/build-wasm.sh > /dev/null
echo "ok: dx-web/src/wasm/pkg/"

step "3/6 JS 문법"
node --check dx-web/src/app.js
node --check dx-web/src/components.js
node --check dx-web/src/bridge.js
node --check dx-web/src/live.js
echo "ok"

step "4/6 좌표·offset 회귀 (verify_fixes)"
(cd dx-web && node verify_fixes.mjs | tail -1)

if [ "$FAST" = "--fast" ]; then
  echo; echo "✔ fast 게이트 통과 (벤치·시각 생략)"; exit 0
fi

step "5/6 렌더 벤치 (회귀 감시 — 수치는 추세 비교용)"
(cd dx-web && node bench_composite.mjs | grep -E "view|ok:")

step "6/6 시각 산출물 (/tmp/dcli-scene-*.png — 에이전트가 Read로 직접 검수)"
(cd dx-web && node render_scene.mjs)

echo; echo "✔ 전체 게이트 통과. 데몬 반영이 필요하면: cargo build -p dcli-daemon && 데몬 재시작"
