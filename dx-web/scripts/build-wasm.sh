#!/usr/bin/env bash
# 코어(dcli-wasm)를 브라우저용 ES module로 빌드한다.
# wasm-pack이 rustc 1.91을 요구해 못 쓰므로 cargo build + wasm-bindgen을 직접 호출.
# wasm-bindgen-cli 버전은 Cargo.toml의 wasm-bindgen과 정확히 일치해야 한다(0.2.122).
set -euo pipefail
cd "$(dirname "$0")/../.."   # repo root

echo "▶ wasm32 release 빌드 (simd128 — 블렌드/샘플 루프 자동 벡터화)"
RUSTFLAGS="${RUSTFLAGS:-} -C target-feature=+simd128" \
  cargo build -p dcli-wasm --target wasm32-unknown-unknown --release

echo "▶ wasm-bindgen (--target web)"
wasm-bindgen target/wasm32-unknown-unknown/release/dcli_wasm.wasm \
  --target web --out-dir dx-web/src/wasm/pkg --out-name dcli_wasm

echo "✔ dx-web/src/wasm/pkg/ 생성 완료"
ls -la dx-web/src/wasm/pkg/
