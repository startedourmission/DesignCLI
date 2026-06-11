# DesignCLI — 개발 루프 가이드

CLI/에이전트로 조작하는 Photoshop류 웹 이미지 에디터. Rust 워크스페이스(headless 코어 +
CLI/daemon/MCP/WASM 셸) + dx-web(Lit, React 금지, 이모지 금지·SVG만).

## 루프 엔지니어링 (수정 → 검증 → 검수)

한 번의 수정 사이클은 반드시 이 순서로 닫는다:

```bash
bash scripts/check.sh          # 전체 게이트: 테스트+wasm빌드+JS+회귀+벤치+시각 산출물
bash scripts/check.sh --fast   # 테스트·회귀만(반복 루프 안쪽)
```

- 시각 검수: check.sh가 `/tmp/dcli-scene-{fit,100,400}.png`를 생성한다. **에이전트는 이
  PNG들을 직접 열어(이미지 Read) 계단현상·위치 어긋남·텍스트 선명도를 판정**할 것.
- 성능 추세: `node dx-web/bench_composite.mjs` 수치를 직전 실행과 비교(기준: 30레이어
  1254² 기준 view fit≈30ms, 100%≈60ms, 확대≈70ms 수준 유지).
- 좌표·offset 불변식 회귀: `node dx-web/verify_fixes.mjs` (전부 PASS여야 함).

## 변경 후 반영 체크리스트 (스테일 = 버그 리포트의 단골 원인)

| 바꾼 것 | 해야 하는 것 |
|---|---|
| crates/* (엔진·dispatch·Shape) | `bash dx-web/scripts/build-wasm.sh` + **데몬 재시작** + 브라우저 강력 새로고침 |
| dx-web/src/*.js | 브라우저 새로고침만 |
| 골든 의도적 변경 | `UPDATE_GOLDEN=1 cargo test -p parity_spike` 후 재실행 통과 확인 |

데몬 재시작: `cargo build -p dcli-daemon && kill $(lsof -tnP -iTCP:8137 -sTCP:LISTEN); target/debug/dx-daemon &`
(문서는 500ms 자동저장, 웹은 자동 재접속 — 재시작 안전)

## 지켜야 할 계약 (어기면 조용히 깨짐)

- **좌표**: `layer_bounds`는 표면 로컬(src) 좌표. meta.item 좌표는 마지막 materialize 시점
  좌표이고 `월드 = 아이템 + (offset − origin(items))`. 재래스터 시 offset은 반드시
  리베이스(app.js `_rebasedOffset`, 엔진 origin 공식 미러 `_itemsOrigin`).
- **비트 경계**: export/PSD/골든은 정확한 powf 경로(`to_srgb8_rgba`), 화면 표시만
  `_fast`(LUT). blend fast path를 바꾸면 wgsl(blend.wgsl)에 1:1 미러 필수(패리티 게이트).
- **블렌딩**: u8 문서는 감마 공간(Photoshop 패리티) — `gamma-vs-linear` 분기 제거 금지.
- **렌더 경로**: 화면은 `composite_view`(보이는 픽셀만 + 벡터 meta 재래스터 캐시),
  export는 `composite_region`(문서 해상도 정본). 두 경로의 시각 결과는 같아야 한다.

## 자주 쓰는 명령

```bash
cargo test --workspace                         # 전체 테스트(골든·패리티 포함)
cargo run -q -p dcli-cli --bin dx -- --help    # CLI 표면
target/debug/dx --server http://localhost:8137 --doc projects/<n>.dxdoc <verb> ...  # 라이브 편집
```

디자인 작업 에이전트 규칙은 `AGENTS.md`·`docs/agent-system-prompt.md`, 아키텍처 배경은
프로젝트 메모리(editor-coordinate-contracts, rendering-architecture) 참조.
