# dx-web — DesignCLI 브라우저 셸 (Phase 6)

코어 Rust 엔진(`dcli-wasm`)을 WASM으로 컴파일해 브라우저에서 직접 돌리는 경량 웹 에디터.
번들러·npm 없이 ESM + import map + Lit(CDN)만 사용한다(메모리 규율: React 금지, 최대한 경량).

## 구조

- `src/wasm/pkg/` — `dcli-wasm`을 `wasm-bindgen --target web`으로 빌드한 산출물(커밋됨).
- `src/main.js` — 진입점. wasm 초기화 → `Editor` 생성 → `App`/`Renderer` 배선.
- `src/app.js` — `AppController`. **Editor(wasm) 핸들이 유일 진실원**, 모든 쓰기는 `apply()` funnel만 통과.
- `src/components.js` — Lit Web Components(툴바·캔버스·레이어 패널·셸).
- `src/bridge.js` — Action/Shape 빌더. `dispatch.rs`의 serde 형태와 1:1(단, `png_path`는 브라우저에 없음).
- `index.html` — `<app-shell>` 마운트 + import map(Lit).

## 빌드 (wasm 재생성)

코어 Rust를 고쳤다면 wasm 산출물을 다시 만든다:

```bash
./scripts/build-wasm.sh
```

요구: `rustup target add wasm32-unknown-unknown` + `wasm-bindgen-cli`가 `Cargo.toml`의
`wasm-bindgen` 버전과 **정확히 일치**(현재 0.2.122). `wasm-pack`은 신형 rustc를 요구해 쓰지 않는다.

## 실행

ESM 상대 import와 wasm fetch 때문에 `file://`로는 안 되고 정적 서버가 필요하다.
**`dx-web/`을 서버 루트로** 띄워야 import 경로가 맞는다:

```bash
cd dx-web
python3 -m http.server 8137
# → http://localhost:8137/   (로컬 자족 모드)
```

## 라이브 모드 (CLI ↔ 웹 실시간 동기화)

`dx-daemon`을 띄우면 CLI/에이전트가 만든 디자인이 웹에 실시간 반영되고, 웹에서 그린 것도
CLI·다른 탭에 반영된다(양방향). 데몬이 `dx-web/` 정적 파일까지 서빙하므로 한 포트면 끝:

```bash
# 1) 데몬 기동(8137). dx-web도 같은 포트에서 서빙됨.
cargo run -p dcli-daemon

# 2) 문서 생성(또는 CLI가 doc create로 만들어도 됨)
curl -X POST 'http://localhost:8137/doc/demo/create?w=800&h=600'

# 3) 브라우저: ?doc=<id> 가 있으면 라이브 모드로 진입
#    http://localhost:8137/?doc=demo

# 4) CLI로 그리면 브라우저에 즉시 등장(새로고침 불필요)
dx --server http://localhost:8137 --doc demo draw rect 100 100 200 150 --color 255,80,80,255
dx --server http://localhost:8137 --doc demo draw ellipse 400 300 80 80 --color 80,160,255,255
```

**동기화 모델:** 데몬이 유일한 순서 결정자. 문서별 단조 증가 `seq`를 둔다. 웹은 접속 시
`.dxpkg` 스냅샷 + 현재 seq를 받아 wasm `Editor`를 초기화하고, 이후 WebSocket으로 오는
`{seq, actions}`(또는 undo/redo)를 순서대로 재적용한다. 웹 편집도 로컬 즉시 적용 대신
데몬에 POST → broadcast로 돌아온 걸 적용하므로 echo 중복·순서꼬임이 없다. `apply_batch`가
결정적이고 `.dxpkg` 라운드트립이 비트동일하므로 모든 클라가 같은 상태로 수렴한다.

**프로토타입 한계:** 낙관적 즉시 렌더 없음(왕복 후 적용), undo/redo는 데몬 단일 History
기준(단일 사용자 가정), 디스크 자동 영속화 없음(데몬 메모리). `--server` 모드에서 `export`/
`--dry-run`은 미지원(웹 UI의 PNG 버튼 사용).

## 검증 (브라우저 없이)

Node ESM에서 wasm `Editor`의 전 기능(apply/undo/redo/dxpkg 라운드트립/png export)을 점검:

```bash
cd dx-web
node verify_wasm.mjs
```

## 불변식

- 코어 4종(color/tile/model/raster)은 UI·플랫폼 무의존 — wasm 어댑터는 `dcli-cli`를
  `default-features=false`로 의존해 clap·`std::fs`(storage, `PngPath`)를 빌드에서 제거한다.
- op 적용·undo·합성은 전부 코어 `History`/`dispatch`를 거친다(JS는 상태를 복제하지 않음).
- 합성 결과는 straight-alpha sRGB8 RGBA로 넘겨 canvas `ImageData`에 그대로 넣는다(premul 누수·detach 회피).
