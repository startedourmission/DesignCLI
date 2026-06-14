# DesignCLI

DesignCLI는 이미지 편집 문서를 CLI, 네이티브 GUI, 웹 UI, AI 에이전트에서 같은 방식으로
조작하기 위한 Rust 기반 디자인 엔진입니다.

핵심 목표는 "Photoshop/Figma 라이트 클론"이 아니라, 디자인 작업을 명령줄과 에이전트가
안정적으로 다룰 수 있는 인터페이스로 만드는 것입니다. 문서 모델과 렌더링은 UI와 분리된
Rust 코어가 소유하고, 각 프론트엔드는 같은 op/dispatch 경로를 호출합니다.

## 주요 기능

- `.dxdoc/` 폴더 문서 포맷: `doc.json` + `pixels/` 사이드카 구조
- `dx` CLI: 문서 생성, 레이어 편집, 도형/텍스트 그리기, PNG/PSD 입출력
- `dx-studio`: CLI와 같은 문서를 여는 네이티브 GUI
- `dx-mcp`: AI 에이전트용 stdio MCP 서버
- `dx-daemon` + `dx-web`: CLI와 브라우저 UI의 실시간 동기화
- CPU 정본 렌더러와 wgpu 렌더러의 픽셀 패리티 테스트
- Photoshop 호환을 고려한 색공간 contract

## 아키텍처

DesignCLI의 기본 규칙은 "상태와 합성 결과의 진실원은 하나"입니다.

```text
           dx CLI
             |
dx-studio -- dispatch/op -- dcli-model -- dcli-raster -- PNG/PSD export
             |                 |
          dx-mcp            dcli-tile
             |
        dx-daemon <--> dx-web (WASM)
```

코어 crate는 UI, 서버, 브라우저 런타임에 의존하지 않습니다. CLI, GUI, MCP, 웹은 모두 같은
문서 모델과 dispatch 엔진을 사용합니다.

## Crate 구성

| crate | 역할 |
| --- | --- |
| `dcli-color` | sRGB EOTF/OETF, premul, bit-depth별 색 contract |
| `dcli-tile` | 픽셀 표면과 타일 저장 단위 |
| `dcli-model` | 문서, 레이어, op, history 모델 |
| `dcli-raster` | CPU 정본 합성기와 도형/텍스트 래스터라이저 |
| `dcli-gpu` | wgpu 기반 윈도리스 합성 및 readback |
| `dcli-cli` | `dx` CLI와 공용 dispatch/storage/dto |
| `dcli-shell-native` | `dx-studio` 네이티브 GUI |
| `dcli-mcp` | `dx-mcp` MCP 서버 |
| `dcli-daemon` | 웹/CLI 실시간 동기화 서버 |
| `dcli-wasm` | 브라우저용 WASM 어댑터 |
| `dcli-psd` | PSD import/export 변환 |

## 빠른 시작

요구사항:

- Rust 1.89+
- macOS/Linux/Windows 중 wgpu가 동작하는 환경
- 웹 셸을 빌드할 경우 `wasm32-unknown-unknown` 타깃과 `wasm-bindgen-cli`

문서를 만들고 PNG로 내보냅니다.

```bash
cargo run -p dcli-cli -- --doc demo.dxdoc doc create --depth u8
cargo run -p dcli-cli -- --doc demo.dxdoc layer add --name bg --fill 245,241,232,255
cargo run -p dcli-cli -- --doc demo.dxdoc draw rounded-rect 80 80 320 180 \
  --radius 20 --color 60,120,220,255 --name card
cargo run -p dcli-cli -- --doc demo.dxdoc draw text 112 145 "Hello DesignCLI" \
  --size 32 --color 255,255,255,255 --name title
cargo run -p dcli-cli -- --doc demo.dxdoc export png out.png
```

빌드된 바이너리를 직접 쓰려면:

```bash
cargo build
target/debug/dx --doc demo.dxdoc layer list
```

## CLI 예시

```bash
# 문서
dx --doc my.dxdoc doc create --depth u8
dx --doc my.dxdoc --json doc info

# 레이어
dx --doc my.dxdoc layer add --name photo --image photo.png
dx --doc my.dxdoc layer add --name tint --fill 80,120,220,180
dx --doc my.dxdoc layer set 2 --opacity 0.7 --x 24 --y -12
dx --doc my.dxdoc blend set 2 multiply
dx --doc my.dxdoc layer list

# 도형과 텍스트
dx --doc my.dxdoc draw rect 0 0 320 120 --color 30,30,30,255
dx --doc my.dxdoc draw ellipse 420 260 80 80 --color 255,210,80,255
dx --doc my.dxdoc draw line 80 80 400 240 --width 6 --color 0,0,0,255
dx --doc my.dxdoc draw text 100 120 "Design from CLI" --size 42

# export
dx --doc my.dxdoc export png out.png
dx --doc my.dxdoc psd export out.psd
dx --doc imported.dxdoc psd import input.psd
```

CLI 규약:

- `--json`: stdout에 구조화 데이터 출력
- `--dry-run`: 변경 결과만 계산하고 저장하지 않음
- 에러: stderr + non-zero exit code

## GUI와 웹 실행

네이티브 GUI:

```bash
cargo run -p dcli-shell-native -- demo.dxdoc
```

웹/라이브 모드:

```bash
cargo run -p dcli-daemon
```

기본 포트는 `8137`입니다.

```bash
curl -X POST 'http://localhost:8137/doc/demo/create'
open 'http://localhost:8137/?doc=demo'

dx --server http://localhost:8137 --doc demo draw rect 100 100 240 160 \
  --color 255,80,80,255
```

`dx-daemon`은 `dx-web/` 정적 파일도 함께 서빙합니다. 문서는 기본적으로 저장소 루트의
`projects/` 아래에 자동 저장됩니다. `DX_PORT`, `DX_PROJECTS`, `DX_WEB_DIR` 환경변수로
포트와 경로를 바꿀 수 있습니다.

웹 WASM 산출물을 다시 만들 때는:

```bash
cd dx-web
./scripts/build-wasm.sh
node verify_wasm.mjs
```

자세한 웹 셸 설명은 [dx-web/README.md](dx-web/README.md)를 참고하세요.

## MCP 서버

`dx-mcp`는 AI 에이전트가 문서를 직접 조작하기 위한 stdio MCP 서버입니다.

```bash
cargo run -p dcli-mcp
```

MCP와 CLI는 같은 dispatch 엔진을 공유합니다. `batch_apply`는 여러 action을 하나의
트랜잭션으로 적용하고, 실패하면 전체를 롤백합니다. `doc_snapshot`은 현재 합성 결과를 PNG로
반환해 에이전트가 편집 결과를 확인할 수 있게 합니다.

새 에이전트 세션이 바로 디자인 작업을 시작해야 할 때는
[DesignCLI Agent System Prompt](docs/agent-system-prompt.md)를 시스템 프롬프트로 사용하세요.

## 렌더링 Contract

초기 리스크는 픽셀 결과가 플랫폼이나 경로마다 달라지는 문제였습니다. DesignCLI는 아래
규칙을 테스트로 고정합니다.

- 8/16bit 문서: Photoshop 호환을 위해 감마 공간 블렌딩
- 32bit 문서: linear-light 블렌딩
- CPU 정본, wgpu 윈도리스 렌더러, 네이티브 프리뷰가 같은 픽셀을 내야 함
- Apple M4/Metal 기준 parity spike에서 CPU와 GPU `max-abs = 0` 확인

검증 명령:

```bash
cargo run -p parity_spike -- --out /tmp/cpu.png
cargo run -p parity_spike -- --linear --out /tmp/linear.png
cargo run -p parity_spike -- --gpu-headless --out /tmp/gpu.png
cargo run -p parity_spike -- --diff
cargo run -p parity_spike -- --preview
```

## 테스트

```bash
cargo test
UPDATE_GOLDEN=1 cargo test -p parity_spike --test parity
```

`dcli-wasm`은 wasm 전용 cdylib이므로 기본 workspace build/test에서 제외되어 있습니다.
브라우저 어댑터를 수정한 경우 `dx-web/scripts/build-wasm.sh`와 `dx-web/verify_wasm.mjs`를
함께 실행하세요.

## 현재 방향

완료된 큰 축은 CLI, 네이티브 셸, MCP, PSD Tier0, 웹 라이브 셸입니다. 다음 작업은 PSD
호환성 확대, 브라우저 편집 UX 강화, 더 많은 Photoshop식 블렌드/텍스트/효과 충실도 개선입니다.
