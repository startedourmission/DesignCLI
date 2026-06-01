# DesignCLI

Photoshop류 이미지 편집을 **CLI/AI 에이전트로 조작**하는 헤드리스 에디터.
목표는 "웹 기반 디자인 도구 + CLI 완벽 지원" — Figma/Photoshop 라이트 클론이 아니라,
그 도구들에 없는 **CLI/에이전트 인터페이스 자체가 제품**.

핵심 아키텍처: UI-비종속 Rust 코어가 문서 모델과 모든 렌더링을 소유하고, CLI·네이티브
셸·(추후) 브라우저 셸은 같은 코어를 호출하는 교체 가능한 프론트엔드. **native-first**로
시작해 브라우저 셸을 코어 변경 0으로 얹는다.

## 현재: Phase 0 — 색 contract + 3경로 픽셀 패리티 (디리스킹 완료)

가장 위험한 두 가지를 day-1에 골든이미지로 박았다:

1. **감마 vs 리니어 합성 분기** — Photoshop은 8/16bit를 *감마 공간*에서 블렌딩한다.
   순수 linear-light 엔진은 모든 표준 블렌드에서 Photoshop과 어긋난다. 그래서 합성
   색공간을 비트깊이로 분기한다(8/16bit=감마, 32bit=리니어). 두 개의 진짜 다른 코드 경로.
2. **한 코어 → 같은 픽셀** — 동일 문서를 CPU 정본 / wgpu 윈도리스 / 네이티브 프리뷰
   세 경로로 렌더해 일치를 검증.

검증 결과 (Apple M4 / Metal): CPU↔GPU **max-abs = 0** (감마·리니어 양 경로).

### crate 구조 (코어 4종은 UI-무의존)

| crate | 책임 |
|-------|------|
| `dcli-color` | sRGB EOTF/OETF(진짜 piecewise)·dequantize(16b=/32768)·premul, 색 contract |
| `dcli-tile`  | 픽셀 표면(linear-premul 저장), 64×64 타일 격자 |
| `dcli-model` | 문서 메타·레이어·blendMode enum (Phase 0 최소판) |
| `dcli-raster`| **CPU 정본 합성기** — 감마/리니어 분기 블렌딩 |
| `dcli-gpu`   | wgpu 윈도리스 합성 + readback (CPU와 동일 셰이딩 수학을 wgsl로 복제) |
| `dcli-cli`   | **`dx` 명령줄 도구** — doc/layer/blend/export verb, --json/--dry-run |
| `dcli-shell-native` | **`dx-studio` 네이티브 GUI** — winit+wgpu+egui, 동일 코어 직링크 |
| `examples/parity_spike` | 3경로 렌더 + 골든/패리티 테스트 + egui 프리뷰 창 |

### `dx` CLI (Phase 2)

문서 = 폴더(`.dxdoc/`): `doc.json`(구조) + `pixels/<id>.bin`(픽셀 사이드카).

```bash
dx --doc my.dxdoc doc create --w 128 --h 96 --depth u8   # 새 문서
dx --doc my.dxdoc layer add --name bg --fill 235,130,40,255
dx --doc my.dxdoc layer add --name top --image photo.png
dx --doc my.dxdoc blend set 1 multiply
dx --doc my.dxdoc layer list
dx --doc my.dxdoc export png out.png

dx --doc my.dxdoc --json doc info       # stdout=JSON (에이전트용)
dx --doc my.dxdoc --dry-run layer add ... # 적용 안 하고 결과만
```

규약: `--json`=stdout 구조화 데이터, 에러=stderr+exit≠0, `--dry-run`=비저장.

### `dx-studio` 네이티브 셸 (Phase 3)

CLI와 **같은 문서 폴더**를 열어 GUI로 편집한다. 모든 편집은 코어 op을 통과하므로
셸·CLI·export가 같은 픽셀을 낸다(이중 상태관리 금지). 레이어 추가/이동/삭제,
opacity·blend·visible 편집, undo/redo, 저장, PNG export.

```bash
dx-studio my.dxdoc      # CLI가 만든 문서를 GUI로 열기
dx-studio               # 빈 문서로 시작
```

### 실행

```bash
cargo run -p parity_spike -- --out /tmp/cpu.png          # CPU 정본 (감마/U8)
cargo run -p parity_spike -- --linear --out /tmp/lin.png # 리니어 (F32)
cargo run -p parity_spike -- --gpu-headless --out /tmp/gpu.png
cargo run -p parity_spike -- --diff                       # CPU vs GPU max-abs
cargo run -p parity_spike -- --preview                    # egui 네이티브 창

cargo test                                                # 전체 게이트 (21 tests)
UPDATE_GOLDEN=1 cargo test -p parity_spike --test parity  # 골든 갱신
```

## 다음 (로드맵)

~~P1 문서모델~~ → ~~P2 CLI verb(`dx`)~~ → ~~P3 네이티브 셸(winit+wgpu+egui)~~ →
**P4 MCP 툴** → P5 PSD Tier0 → P6 브라우저 셸 → P7 충실도 확장.
