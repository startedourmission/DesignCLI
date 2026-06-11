//! `dx` — DesignCLI 명령줄 인터페이스.
//!
//! CLI 서브커맨드는 코어 op과 1:1 대응한다(cli-agent-interface: CLI verb ≡ MCP tool).
//! 횡단 플래그: --doc(작업 대상 폴더), --json(stdout=데이터), --dry-run(적용될 변경만).
//!
//! 작업 흐름: 대부분의 명령은 문서 폴더를 load → op 적용 → save 한다. --dry-run이면
//! 적용 결과를 보여주되 save하지 않는다.

mod output;

use dcli_cli::client::Server;
use dcli_cli::dispatch::{
    self, Action, BatchResult, BlendModeDto, NodeRef, PixelSource, PropPatch, Shape,
};
use dcli_cli::storage;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dcli_color::BitDepth;
use dcli_model::{Document, History, NodeId};
use output::Emitter;
use std::path::PathBuf;
use storage::DocPath;

#[derive(Parser)]
#[command(
    name = "dx",
    version,
    about = "DesignCLI — CLI로 조작하는 이미지 에디터"
)]
struct Cli {
    /// 작업 대상 문서 폴더(.dxdoc). 대부분의 명령이 사용.
    /// --server 모드에서는 이 문자열(파일명 stem)이 데몬의 문서 id가 된다.
    /// 환경변수 DX_DOC이 기본값(에디터 내장 터미널이 주입 — 플래그 생략 가능).
    #[arg(long, global = true, env = "DX_DOC", default_value = "doc.dxdoc")]
    doc: PathBuf,

    /// 라이브 데몬(dx-daemon) URL. 지정 시 디스크 대신 데몬에 편집을 보낸다
    /// (웹 UI에 실시간 반영). 예: --server http://localhost:8137
    /// 환경변수 DX_SERVER가 기본값 — 내장 터미널에선 모든 dx 명령이 자동 라이브.
    #[arg(long, global = true, env = "DX_SERVER")]
    server: Option<String>,

    /// 데이터를 JSON으로 stdout에 출력(에이전트 친화).
    #[arg(long, global = true)]
    json: bool,

    /// 적용될 변경만 보여주고 저장하지 않음.
    #[arg(long, global = true)]
    dry_run: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 문서 명령.
    #[command(subcommand)]
    Doc(DocCmd),
    /// 레이어 명령.
    #[command(subcommand)]
    Layer(LayerCmd),
    /// 블렌드 명령.
    #[command(subcommand)]
    Blend(BlendCmd),
    /// 도형 그리기(새 레이어로).
    #[command(subcommand)]
    Draw(DrawCmd),
    /// 합성 결과를 파일로 export.
    #[command(subcommand)]
    Export(ExportCmd),
    /// Frame(명명된 export 영역 — Figma의 캔버스) 관리.
    #[command(subcommand)]
    Frame(FrameCmd),
    /// PSD 호환 — import(PSD→문서)/export(문서→PSD).
    #[command(subcommand)]
    Psd(PsdCmd),
    /// 사용 가능한 글꼴 목록(번들 + 시스템 + ./fonts).
    #[command(subcommand)]
    Font(FontCmd),
    /// 마지막 편집을 되돌린다(--server 모드 전용 — 디스크 모드는 세션 히스토리 없음).
    Undo,
    /// 되돌린 편집을 다시 적용한다(--server 모드 전용).
    Redo,
}

#[derive(Subcommand)]
enum FrameCmd {
    /// Frame 추가: 이름 + 영역(x y w h, 음수 좌표 허용 — 무한 작업영역).
    Add {
        name: String,
        #[arg(allow_negative_numbers = true)]
        x: i32,
        #[arg(allow_negative_numbers = true)]
        y: i32,
        w: u32,
        h: u32,
    },
    /// Frame 목록.
    List,
    /// Frame 수정(이동/크기/이름 — 이름 또는 id로 찾기).
    Set {
        name: String,
        #[arg(long, allow_negative_numbers = true)]
        x: Option<i32>,
        #[arg(long, allow_negative_numbers = true)]
        y: Option<i32>,
        #[arg(long)]
        w: Option<u32>,
        #[arg(long)]
        h: Option<u32>,
        /// 새 이름.
        #[arg(long)]
        rename: Option<String>,
    },
    /// Frame 제거(이름 또는 id).
    Remove { name: String },
}

#[derive(Subcommand)]
enum FontCmd {
    /// 글꼴 이름 목록(시스템 스캔 — 최초 1회 수 초 걸릴 수 있음).
    List,
}

#[derive(Subcommand)]
enum PsdCmd {
    /// PSD 파일을 읽어 현재 --doc 경로에 새 문서로 변환 저장.
    Import { input: PathBuf },
    /// 현재 문서를 PSD로 저장(레이어별 트랜스폼 베이크 + 합성 이미지 포함).
    Export { out: PathBuf },
}

#[derive(Subcommand)]
enum DrawCmd {
    /// 채워진 사각형: 좌상단 (x,y) 크기 (w,h).
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        /// 색 "R,G,B,A" (0-255).
        #[arg(long, default_value = "0,0,0,255")]
        color: String,
        #[arg(long, default_value = "rect")]
        name: String,
    },
    /// 채워진 타원: 중심 (cx,cy) 반지름 (rx,ry).
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        #[arg(long, default_value = "0,0,0,255")]
        color: String,
        #[arg(long, default_value = "ellipse")]
        name: String,
    },
    /// 선분: (x0,y0)→(x1,y1) 두께 width.
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        #[arg(long, default_value_t = 1.0)]
        width: f32,
        #[arg(long, default_value = "0,0,0,255")]
        color: String,
        #[arg(long, default_value = "line")]
        name: String,
    },
    /// 테두리 사각형(외곽선만): 좌상단 (x,y) 크기 (w,h), 두께 --width.
    StrokeRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        #[arg(long, default_value_t = 2.0)]
        width: f32,
        #[arg(long, default_value = "0,0,0,255")]
        color: String,
        #[arg(long, default_value = "stroke-rect")]
        name: String,
    },
    /// 테두리 타원(링만): 중심 (cx,cy) 반지름 (rx,ry), 두께 --width.
    StrokeEllipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        #[arg(long, default_value_t = 2.0)]
        width: f32,
        #[arg(long, default_value = "0,0,0,255")]
        color: String,
        #[arg(long, default_value = "stroke-ellipse")]
        name: String,
    },
    /// 텍스트: 좌상단 (x,y), 내용 TEXT. '\n' 줄바꿈. --font로 시스템/로컬 글꼴 선택.
    Text {
        x: f32,
        y: f32,
        text: String,
        #[arg(long, default_value_t = 24.0)]
        size: f32,
        #[arg(long, default_value = "0,0,0,255")]
        color: String,
        /// 글꼴 이름(dx font list 참조). 미지정/미발견 = 번들 Pretendard.
        #[arg(long)]
        font: Option<String>,
        #[arg(long, default_value = "text")]
        name: String,
    },
    /// 모서리 둥근 사각형: 좌상단 (x,y) 크기 (w,h), 반지름 --radius.
    RoundedRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        #[arg(long, default_value_t = 8.0)]
        radius: f32,
        #[arg(long, default_value = "0,0,0,255")]
        color: String,
        #[arg(long, default_value = "rounded-rect")]
        name: String,
    },
    /// 자유곡선(브러시): 점 목록 "x,y x,y x,y ..." 를 둥근 끝 선분으로 연결.
    Path {
        /// 공백 구분 점 목록. 예: "10,10 50,30 90,20"
        points: String,
        #[arg(long, default_value_t = 4.0)]
        width: f32,
        #[arg(long, default_value = "0,0,0,255")]
        color: String,
        #[arg(long, default_value = "path")]
        name: String,
    },
}

#[derive(Subcommand)]
enum DocCmd {
    /// 새 문서를 생성한다.
    Create {
        #[arg(long, default_value_t = 512)]
        w: u32,
        #[arg(long, default_value_t = 512)]
        h: u32,
        /// 비트깊이: u8(감마 합성) | u16 | f32(리니어 합성).
        #[arg(long, default_value = "u8")]
        depth: String,
    },
    /// 문서 메타 정보를 출력한다(희소).
    Info,
    /// 구버전/비대한 텍스트 표면을 내용 크기로 압축해 저장한다.
    Compact,
}

#[derive(Subcommand)]
enum LayerCmd {
    /// 페인트 레이어를 추가한다. --image로 PNG를 불러오거나, 없으면 단색/투명.
    Add {
        #[arg(long, default_value = "layer")]
        name: String,
        /// 불러올 PNG 경로(문서 크기와 일치해야 함).
        #[arg(long)]
        image: Option<PathBuf>,
        /// 단색 채우기 "R,G,B,A" (0-255). --image 없을 때.
        #[arg(long)]
        fill: Option<String>,
        /// 그라데이션 채우기 "R,G,B,A:R,G,B,A" (시작:끝, 문서 전체).
        #[arg(long)]
        gradient: Option<String>,
        /// 그라데이션 각도(도). 90 = 위→아래.
        #[arg(long, default_value_t = 90.0, allow_negative_numbers = true)]
        gradient_angle: f32,
        /// 방사형 그라데이션.
        #[arg(long)]
        gradient_radial: bool,
        /// 삽입 위치(bottom-to-top 인덱스, 없으면 맨 위).
        #[arg(long)]
        index: Option<usize>,
    },
    /// 레이어 재스타일(채움 없음/단색/그라데이션·테두리·반경·그림자) — 노드 보존.
    Style {
        id: u64,
        /// 채움 단색 "R,G,B,A" (그라데이션 해제).
        #[arg(long)]
        fill: Option<String>,
        /// 채움 제거(테두리/그림자만 남김).
        #[arg(long)]
        no_fill: bool,
        /// 채움 그라데이션 "R,G,B,A:R,G,B,A" (시작:끝).
        #[arg(long)]
        gradient: Option<String>,
        /// 그라데이션 각도(도). 90 = 위→아래.
        #[arg(long, default_value_t = 90.0, allow_negative_numbers = true)]
        gradient_angle: f32,
        /// 방사형 그라데이션(중심→가장자리).
        #[arg(long)]
        gradient_radial: bool,
        /// 테두리 색 "R,G,B,A".
        #[arg(long)]
        stroke: Option<String>,
        /// 테두리 두께(px). 0 = 제거.
        #[arg(long)]
        stroke_width: Option<f32>,
        /// 테두리 제거.
        #[arg(long)]
        no_stroke: bool,
        /// 코너 반경(px, rect 계열).
        #[arg(long)]
        radius: Option<f32>,
        /// 그림자 "dx,dy,blur,R,G,B,A".
        #[arg(long)]
        shadow: Option<String>,
        /// 그림자 제거.
        #[arg(long)]
        no_shadow: bool,
    },
    /// 텍스트 레이어 편집(내용/크기/색/배경) — 노드 보존 재래스터.
    Text {
        id: u64,
        /// 새 텍스트 내용('\n' 줄바꿈).
        #[arg(long)]
        text: Option<String>,
        /// 폰트 크기(px).
        #[arg(long)]
        size: Option<f32>,
        /// 글자색 "R,G,B,A".
        #[arg(long)]
        color: Option<String>,
        /// 글꼴 이름(dx font list). 빈 문자열 = 번들 폰트로 복귀.
        #[arg(long)]
        font: Option<String>,
        /// 배경 박스 색 "R,G,B,A".
        #[arg(long)]
        bg: Option<String>,
        /// 배경 제거.
        #[arg(long)]
        no_bg: bool,
        /// 배경 코너 반경(px).
        #[arg(long)]
        bg_radius: Option<f32>,
        /// 배경 패딩(px).
        #[arg(long)]
        bg_pad: Option<f32>,
    },
    /// 레이어 목록(bottom-to-top).
    List,
    /// 단일 레이어 상세.
    Get { id: u64 },
    /// 레이어 속성 변경(opacity/visible/name/위치). --x/--y는 함께 줘야 한다.
    Set {
        id: u64,
        #[arg(long)]
        opacity: Option<f32>,
        #[arg(long)]
        visible: Option<bool>,
        #[arg(long)]
        name: Option<String>,
        /// 캔버스 X 평행이동(절대 offset, 픽셀). --y와 함께.
        #[arg(long, requires = "y", allow_negative_numbers = true)]
        x: Option<i32>,
        /// 캔버스 Y 평행이동(절대 offset, 픽셀). --x와 함께.
        #[arg(long, requires = "x", allow_negative_numbers = true)]
        y: Option<i32>,
        /// 비파괴 X 스케일(표면 중심 기준, 음수=뒤집기). --scale-y와 함께.
        #[arg(long, requires = "scale_y", allow_negative_numbers = true)]
        scale_x: Option<f32>,
        /// 비파괴 Y 스케일. --scale-x와 함께.
        #[arg(long, requires = "scale_x", allow_negative_numbers = true)]
        scale_y: Option<f32>,
        /// 비파괴 회전(도, 시계방향, 표면 중심 기준).
        #[arg(long, allow_negative_numbers = true)]
        rotation: Option<f32>,
        /// 임의 메타데이터(JSON 문자열 관행). 빈 문자열("")이면 제거.
        #[arg(long)]
        meta: Option<String>,
    },
    /// 레이어를 새 순서 인덱스로 이동.
    Move { id: u64, to: usize },
    /// 레이어 삭제.
    Delete { id: u64 },
    /// 레이어 복제(표면+속성 복사, offset +12px).
    Duplicate { id: u64 },
    /// 레이어들을 그룹으로 묶는다(bottom-to-top 상대 순서 유지).
    Group {
        ids: Vec<u64>,
        #[arg(long, default_value = "group")]
        name: String,
    },
    /// 그룹 해제 — 자식들을 그룹 위치에 펼친다.
    Ungroup { id: u64 },
}

#[derive(Subcommand)]
enum BlendCmd {
    /// 레이어 블렌드 모드 변경: normal|multiply|screen|darken|lighten|overlay|difference.
    Set { id: u64, mode: String },
}

#[derive(Subcommand)]
enum ExportCmd {
    /// 합성 결과를 PNG로 저장. --frame 또는 --region으로 영역 지정(무한 작업영역).
    Png {
        out: PathBuf,
        /// Frame 이름 또는 id 단위 export.
        #[arg(long)]
        frame: Option<String>,
        /// 임의 영역 "x,y,w,h" (음수 좌표 허용).
        #[arg(long, allow_hyphen_values = true)]
        region: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    let emit = Emitter::new(cli.json);
    if let Err(e) = run(&cli, &emit) {
        emit.error(&e);
        std::process::exit(1);
    }
}

fn parse_depth(s: &str) -> Result<BitDepth> {
    match s {
        "u8" => Ok(BitDepth::U8),
        "u16" => Ok(BitDepth::U16),
        "f32" => Ok(BitDepth::F32),
        other => anyhow::bail!("알 수 없는 비트깊이: {other} (u8|u16|f32)"),
    }
}

fn parse_blend(s: &str) -> Result<BlendModeDto> {
    match s {
        "normal" => Ok(BlendModeDto::Normal),
        "multiply" => Ok(BlendModeDto::Multiply),
        "screen" => Ok(BlendModeDto::Screen),
        "darken" => Ok(BlendModeDto::Darken),
        "lighten" => Ok(BlendModeDto::Lighten),
        "overlay" => Ok(BlendModeDto::Overlay),
        "difference" => Ok(BlendModeDto::Difference),
        other => anyhow::bail!(
            "알 수 없는 블렌드 모드: {other} (normal|multiply|screen|darken|lighten|overlay|difference)"
        ),
    }
}

/// "R,G,B,A:R,G,B,A" + 각도(도) → bbox 상대(0~1) GradFill.
fn parse_gradient(spec: &str, angle_deg: f32, radial: bool) -> Result<dispatch::GradFill> {
    let (a, b) = spec
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("그라데이션은 \"R,G,B,A:R,G,B,A\" 형식"))?;
    let c1 = parse_rgba(a)?;
    let c2 = parse_rgba(b)?;
    let r = angle_deg.to_radians();
    let (dx, dy) = (r.cos() / 2.0, r.sin() / 2.0);
    Ok(dispatch::GradFill {
        x0: 0.5 - dx,
        y0: 0.5 - dy,
        x1: 0.5 + dx,
        y1: 0.5 + dy,
        radial,
        stops: vec![
            dispatch::GradientStop { at: 0.0, rgba: c1 },
            dispatch::GradientStop { at: 1.0, rgba: c2 },
        ],
    })
}

/// "dx,dy,blur,R,G,B,A" → meta.shadow JSON.
fn parse_shadow(spec: &str) -> Result<serde_json::Value> {
    let v: Vec<f32> = spec
        .split(',')
        .map(|t| t.trim().parse::<f32>())
        .collect::<Result<_, _>>()
        .map_err(|_| anyhow::anyhow!("그림자는 \"dx,dy,blur,R,G,B,A\" 형식"))?;
    anyhow::ensure!(v.len() == 7, "그림자는 \"dx,dy,blur,R,G,B,A\" 형식");
    Ok(serde_json::json!({
        "dx": v[0], "dy": v[1], "blur": v[2],
        "rgba": [v[3].clamp(0.0,255.0) as u8, v[4].clamp(0.0,255.0) as u8, v[5].clamp(0.0,255.0) as u8, v[6].clamp(0.0,255.0) as u8],
    }))
}

/// 재스타일 대상 노드 정보: (meta JSON, offset, 표면 크기, 문서 크기).
fn read_node_info(cli: &Cli, path: &DocPath, id: u64) -> Result<(String, (i32, i32), Option<(u32, u32)>, (u32, u32))> {
    if let Some(srv) = server_of(cli) {
        let st = srv.state()?;
        let docv = &st["doc"];
        let dims = (docv["w"].as_u64().unwrap_or(0) as u32, docv["h"].as_u64().unwrap_or(0) as u32);
        fn find<'a>(layers: &'a [serde_json::Value], id: u64) -> Option<&'a serde_json::Value> {
            for l in layers {
                if l["id"].as_u64() == Some(id) {
                    return Some(l);
                }
                if let Some(ch) = l["children"].as_array() {
                    if let Some(f) = find(ch, id) {
                        return Some(f);
                    }
                }
            }
            None
        }
        let layers = st["layers"].as_array().cloned().unwrap_or_default();
        let node = find(&layers, id).ok_or_else(|| anyhow::anyhow!("레이어 없음: n{id}"))?;
        let meta = node["meta"].as_str().unwrap_or("").to_string();
        anyhow::ensure!(!meta.is_empty(), "n{id}에 편집 meta가 없음(레거시 레이어) — draw로 다시 만들거나 웹에서 한 번 색을 바꿔 마이그레이션");
        let off = node["offset"].as_array().map(|a| (a[0].as_i64().unwrap_or(0) as i32, a[1].as_i64().unwrap_or(0) as i32)).unwrap_or((0, 0));
        let ss = node["surface_size"].as_array().map(|a| (a[0].as_u64().unwrap_or(0) as u32, a[1].as_u64().unwrap_or(0) as u32));
        return Ok((meta, off, ss, dims));
    }
    let doc = path.load()?;
    let node = doc.get(NodeId(id)).ok_or_else(|| anyhow::anyhow!("레이어 없음: n{id}"))?;
    let meta = node.meta.clone().unwrap_or_default();
    anyhow::ensure!(!meta.is_empty(), "n{id}에 편집 meta가 없음(레거시 레이어) — draw로 다시 만들거나 웹에서 한 번 색을 바꿔 마이그레이션");
    let ss = node.surface_id().and_then(|sid| doc.pixels().get(sid)).map(|sf| (sf.width(), sf.height()));
    Ok((meta, node.offset, ss, (doc.width, doc.height)))
}

/// meta 변형 → 아이템 재구성 → 위치 보존 리베이스 → replace_paint_source 적용(노드 보존).
fn restyle_layer(
    cli: &Cli,
    path: &DocPath,
    id: u64,
    mutate: impl FnOnce(&mut serde_json::Value) -> Result<()>,
) -> Result<serde_json::Value> {
    let (meta_str, offset, surface_size, doc_dims) = read_node_info(cli, path, id)?;
    let mut m: serde_json::Value =
        serde_json::from_str(&meta_str).map_err(|e| anyhow::anyhow!("meta 파싱 실패: {e}"))?;
    let old_items = dispatch::items_from_meta(&m);
    mutate(&mut m)?;
    if let Some(f) = m.get("font").and_then(|f| f.as_str()) {
        let _ = dcli_raster::sysfonts::ensure(f); // 미발견은 번들 폴백.
    }
    let new_items = dispatch::items_from_meta(&m)
        .ok_or_else(|| anyhow::anyhow!("이 meta로는 아이템을 재구성할 수 없음"))?;
    let doc_sized = surface_size == Some(doc_dims);
    let oc = if doc_sized {
        (0, 0)
    } else {
        old_items
            .as_deref()
            .and_then(dispatch::shapes_origin)
            .unwrap_or((0, 0))
    };
    let on = dispatch::shapes_origin(&new_items).unwrap_or((0, 0));
    let new_offset = (offset.0 + on.0 - oc.0, offset.1 + on.1 - oc.1);
    let actions = vec![
        Action::ReplacePaintSource {
            id: NodeRef::Node(id),
            source: PixelSource::Shapes { items: new_items },
        },
        Action::SetProps {
            id: NodeRef::Node(id),
            patch: PropPatch {
                meta: Some(m.to_string()),
                offset: Some(new_offset),
                ..Default::default()
            },
        },
    ];
    apply_actions(cli, path, actions)?;
    Ok(m)
}

fn parse_rgba(s: &str) -> Result<[u8; 4]> {
    let parts: Vec<&str> = s.split(',').collect();
    anyhow::ensure!(parts.len() == 4, "fill은 'R,G,B,A' 형식 (0-255)");
    let v: Vec<u8> = parts
        .iter()
        .map(|p| p.trim().parse::<u8>())
        .collect::<Result<_, _>>()
        .context("fill 값은 0-255 정수")?;
    Ok([v[0], v[1], v[2], v[3]])
}

/// DrawCmd를 dispatch Shape + 레이어 이름으로 변환.
fn draw_to_shape(cmd: &DrawCmd) -> Result<(Shape, String)> {
    Ok(match cmd {
        DrawCmd::Rect {
            x,
            y,
            w,
            h,
            color,
            name,
        } => (
            Shape::Rect {
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                rgba: parse_rgba(color)?,
                gradient: None,
            },
            name.clone(),
        ),
        DrawCmd::Ellipse {
            cx,
            cy,
            rx,
            ry,
            color,
            name,
        } => (
            Shape::Ellipse {
                cx: *cx,
                cy: *cy,
                rx: *rx,
                ry: *ry,
                rgba: parse_rgba(color)?,
                gradient: None,
            },
            name.clone(),
        ),
        DrawCmd::Line {
            x0,
            y0,
            x1,
            y1,
            width,
            color,
            name,
        } => (
            Shape::Line {
                x0: *x0,
                y0: *y0,
                x1: *x1,
                y1: *y1,
                width: *width,
                rgba: parse_rgba(color)?,
            },
            name.clone(),
        ),
        DrawCmd::StrokeRect {
            x,
            y,
            w,
            h,
            width,
            color,
            name,
        } => (
            Shape::StrokeRect {
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                width: *width,
                rgba: parse_rgba(color)?,
            },
            name.clone(),
        ),
        DrawCmd::StrokeEllipse {
            cx,
            cy,
            rx,
            ry,
            width,
            color,
            name,
        } => (
            Shape::StrokeEllipse {
                cx: *cx,
                cy: *cy,
                rx: *rx,
                ry: *ry,
                width: *width,
                rgba: parse_rgba(color)?,
            },
            name.clone(),
        ),
        DrawCmd::RoundedRect {
            x,
            y,
            w,
            h,
            radius,
            color,
            name,
        } => (
            Shape::RoundedRect {
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                radius: *radius,
                rgba: parse_rgba(color)?,
                gradient: None,
            },
            name.clone(),
        ),
        DrawCmd::Text {
            x,
            y,
            text,
            size,
            color,
            font,
            name,
        } => (
            Shape::Text {
                x: *x,
                y: *y,
                text: text.clone(),
                size: *size,
                rgba: parse_rgba(color)?,
                font: font.clone(),
            },
            // 기본 이름이면 텍스트 내용을 레이어 이름으로(패널 가독성).
            if name == "text" {
                text.chars().take(20).collect()
            } else {
                name.clone()
            },
        ),
        DrawCmd::Path {
            points,
            width,
            color,
            name,
        } => {
            // "x,y x,y ..." → 평탄 좌표 배열.
            let mut flat = Vec::new();
            for pair in points.split_whitespace() {
                let (x, y) = pair
                    .split_once(',')
                    .ok_or_else(|| anyhow::anyhow!("점은 'x,y' 형식: {pair}"))?;
                flat.push(x.trim().parse::<f32>().context("x 좌표")?);
                flat.push(y.trim().parse::<f32>().context("y 좌표")?);
            }
            anyhow::ensure!(flat.len() >= 2, "점이 최소 1개 필요");
            (
                Shape::Path {
                    points: flat,
                    width: *width,
                    rgba: parse_rgba(color)?,
                },
                name.clone(),
            )
        }
    })
}

/// 명시적 --server 모드면 데몬 문서 id를 만든다. id = --doc 경로의 파일명 stem
/// (예: "demo.dxdoc" → "demo", "demo" → "demo").
fn explicit_server_of(cli: &Cli) -> Option<Server> {
    let base = cli.server.as_ref()?;
    let id = cli
        .doc
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("doc")
        .to_string();
    Some(Server::new(base, &id))
}

/// 실제 작업 대상 서버.
///
/// 사용자가 --server를 명시하면 그 서버를 사용한다. 명시하지 않았더라도 로컬 데몬에서
/// `projects/<id>.dxdoc`가 이미 열린 상태면 디스크 저장 대신 데몬에 적용해 웹 UI와
/// 레이어 패널이 즉시 같은 상태를 보게 한다.
///
/// 자동 감지는 프로세스당 1회만 프로브한다(OnceLock) — 명령 하나가 apply와 출력 라벨에서
/// server_of를 여러 번 부르므로, 캐시 없이는 HTTP 왕복이 배가되고 두 호출 사이에 데몬
/// 상태가 바뀌면 실제 기록 위치와 라벨이 어긋난다(TOCTOU).
/// --dry-run은 자동 승격하지 않는다: 데몬 모드는 dry-run을 지원하지 않아, 웹에서 문서를
/// 열어둔 것만으로 검증용 스크립트가 깨지거나 실제 쓰기가 일어나면 안 된다.
fn server_of(cli: &Cli) -> Option<Server> {
    if let Some(srv) = explicit_server_of(cli) {
        return Some(srv);
    }
    if cli.dry_run {
        return None;
    }
    static AUTO: std::sync::OnceLock<Option<Server>> = std::sync::OnceLock::new();
    AUTO.get_or_init(|| Server::auto_for_open_project(&cli.doc))
        .clone()
}

fn write_target(cli: &Cli) -> Option<&'static str> {
    if cli.dry_run {
        None
    } else if server_of(cli).is_some() {
        Some("live")
    } else {
        Some("disk")
    }
}

/// 한 Action을 적용한다(CLI 쓰기 공통 경로). --server면 데몬에, 아니면 디스크에.
/// 둘 다 dispatch::apply_batch 동일 엔진을 거치고 같은 BatchResult를 돌려준다.
fn apply_actions(cli: &Cli, path: &DocPath, actions: Vec<Action>) -> Result<BatchResult> {
    if let Some(srv) = server_of(cli) {
        // 데몬 모드: dry-run은 의미가 다르므로(서버 상태 불변) 그대로 검증만은 미지원 —
        // 프로토타입은 실제 적용. dry-run 플래그는 디스크 모드 전용으로 둔다.
        if cli.dry_run {
            anyhow::bail!("--dry-run은 --server 모드에서 미지원(프로토타입)");
        }
        let res = srv.apply(&actions)?;
        if !res.ok {
            anyhow::bail!(
                "{}",
                res.issues
                    .first()
                    .map(|i| i.message.clone())
                    .unwrap_or_else(|| "적용 실패".into())
            );
        }
        return Ok(res);
    }
    // 디스크 모드.
    let doc = path.load()?;
    let mut h = History::new(doc);
    let res = dispatch::apply_batch(&mut h, &actions, cli.dry_run);
    if !res.ok {
        let issue = res.issues.first();
        anyhow::bail!(
            "{}",
            issue
                .map(|i| i.message.clone())
                .unwrap_or_else(|| "적용 실패".into())
        );
    }
    if !cli.dry_run {
        path.save(&h.doc)?;
    }
    Ok(res)
}

/// 단발 Action 편의 래퍼.
fn apply_one(cli: &Cli, path: &DocPath, action: Action) -> Result<BatchResult> {
    apply_actions(cli, path, vec![action])
}

fn run(cli: &Cli, emit: &Emitter) -> Result<()> {
    let path = DocPath::new(cli.doc.clone());
    match &cli.command {
        Command::Doc(DocCmd::Create { w, h, depth }) => {
            let depth_bd = parse_depth(depth)?;
            if let Some(srv) = server_of(cli) {
                // 데몬에 문서 등록(이미 있으면 멱등). 데몬이 진실원. dry-run은 호출 생략.
                if !cli.dry_run {
                    srv.ensure_doc(*w, *h, depth)?;
                }
                let doc = Document::new(*w, *h, depth_bd);
                emit.doc_created_target(&cli.doc, &doc, cli.dry_run, Some("live"));
                return Ok(());
            }
            anyhow::ensure!(
                !path.exists() || cli.dry_run,
                "이미 문서가 존재: {}",
                cli.doc.display()
            );
            let doc = Document::new(*w, *h, depth_bd);
            if !cli.dry_run {
                path.save(&doc)?;
            }
            emit.doc_created_target(&cli.doc, &doc, cli.dry_run, write_target(cli));
            Ok(())
        }
        Command::Doc(DocCmd::Info) => {
            if let Some(srv) = server_of(cli) {
                // 데몬 state에서 doc 메타만 추려 그대로 출력(JSON 우선).
                let st = srv.state()?;
                emit.raw_json_or(&st["doc"], "doc info (server)");
                return Ok(());
            }
            let doc = path.load()?;
            emit.doc_info(&doc);
            Ok(())
        }
        Command::Doc(DocCmd::Compact) => {
            // 디스크 전용 동작 — 데몬에 열린 문서면 디스크/메모리가 갈라지므로 거부한다
            // (자동 감지 포함). 라벨도 실제 기록 위치(disk)로 정직하게.
            anyhow::ensure!(
                server_of(cli).is_none(),
                "doc compact는 디스크 모드 전용(데몬에 열린 문서는 먼저 닫기)"
            );
            let mut doc = path.load()?;
            let changed = dispatch::compact_text_surfaces(&mut doc);
            if changed > 0 && !cli.dry_run {
                path.save(&doc)?;
            }
            emit.ok_target(
                &format!("문서 압축: 텍스트 표면 {changed}개"),
                cli.dry_run,
                if cli.dry_run { None } else { Some("disk") },
            );
            Ok(())
        }
        Command::Layer(cmd) => run_layer(cli, emit, &path, cmd),
        Command::Blend(BlendCmd::Set { id, mode }) => {
            let mode = parse_blend(mode)?;
            apply_one(
                cli,
                &path,
                Action::SetBlend {
                    id: NodeRef::Node(*id),
                    mode,
                },
            )?;
            emit.ok_target(
                &format!("블렌드 설정: n{id} = {mode:?}"),
                cli.dry_run,
                write_target(cli),
            );
            Ok(())
        }
        Command::Draw(cmd) => {
            if let DrawCmd::Text { font: Some(f), .. } = cmd {
                if !f.is_empty() {
                    anyhow::ensure!(
                        dcli_raster::sysfonts::ensure(f),
                        "글꼴을 찾을 수 없음: {f} (dx font list로 확인)"
                    );
                }
            }
            let (shape, name) = draw_to_shape(cmd)?;
            // 한 도형을 새 레이어로 그린다(layer add의 Shapes source).
            let mut actions = vec![Action::AddPaintLayer {
                name: name.clone(),
                source: PixelSource::Shapes {
                    items: vec![shape.clone()],
                },
                index: None,
                bind: Some("new".into()),
            }];
            // 편집용 meta를 자동 저장(웹 색상/텍스트 편집과 호환).
            let meta = match &shape {
                Shape::Text {
                    x,
                    y,
                    text,
                    size,
                    rgba,
                    font,
                } => serde_json::json!({
                    "type": "text", "x": x, "y": y, "text": text, "size": size, "rgba": rgba,
                    "font": font,
                }),
                Shape::Path { rgba, .. } => serde_json::json!({
                    "type": "brush", "item": shape, "rgba": rgba,
                }),
                Shape::Rect { rgba, .. }
                | Shape::Ellipse { rgba, .. }
                | Shape::Line { rgba, .. }
                | Shape::StrokeRect { rgba, .. }
                | Shape::StrokeEllipse { rgba, .. }
                | Shape::Shadow { rgba, .. } => serde_json::json!({
                    "type": "shape", "shape": name, "item": shape, "fill": rgba, "rgba": rgba,
                    "stroke": null, "strokeWidth": 0, "radius": 0,
                }),
                Shape::RoundedRect { rgba, radius, .. } => serde_json::json!({
                    "type": "shape", "shape": "rect", "item": shape, "fill": rgba, "rgba": rgba,
                    "stroke": null, "strokeWidth": 0, "radius": radius,
                }),
                Shape::StrokeRoundedRect {
                    rgba,
                    radius,
                    width,
                    ..
                } => serde_json::json!({
                    "type": "shape", "shape": "rect", "item": shape, "fill": rgba, "rgba": rgba,
                    "stroke": rgba, "strokeWidth": width, "radius": radius,
                }),
            }
            .to_string();
            actions.push(Action::SetProps {
                id: NodeRef::Bind("new".into()),
                patch: PropPatch {
                    meta: Some(meta),
                    ..Default::default()
                },
            });
            apply_actions(cli, &path, actions)?;
            emit.ok_target(
                &format!("도형 그림: \"{name}\""),
                cli.dry_run,
                write_target(cli),
            );
            Ok(())
        }
        Command::Export(ExportCmd::Png { out, frame, region }) => {
            // --server면 데몬이 합성·인코딩한 PNG를 받아 저장(디스크 모드와 동일 인코딩 경로).
            // dry-run은 실제 파일 쓰기가 일어나므로 데몬 경로에서 명시적으로 거부.
            if let Some(srv) = server_of(cli) {
                anyhow::ensure!(!cli.dry_run, "--dry-run은 --server export 미지원");
                let (w, h) = srv.export_png_with(out, frame.as_deref(), region.as_deref())?;
                emit.exported_target(out, w, h, false, Some("live"));
                return Ok(());
            }
            let doc = path.load()?;
            let surface = if let Some(key) = frame {
                let f = doc
                    .find_frame(key)
                    .ok_or_else(|| anyhow::anyhow!("frame 없음: {key}"))?;
                dcli_raster::composite_region(&doc, f.x, f.y, f.w, f.h)
            } else if let Some(r) = region {
                let v: Vec<i64> = r.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                anyhow::ensure!(v.len() == 4 && v[2] > 0 && v[3] > 0, "region은 x,y,w,h");
                dcli_raster::composite_region(
                    &doc,
                    v[0] as i32,
                    v[1] as i32,
                    v[2] as u32,
                    v[3] as u32,
                )
            } else {
                dcli_raster::composite(&doc)
            };
            if !cli.dry_run {
                storage::export_png(out, &surface)?;
            }
            emit.exported_target(
                out,
                surface.width(),
                surface.height(),
                cli.dry_run,
                if cli.dry_run { None } else { Some("disk") },
            );
            Ok(())
        }
        Command::Frame(cmd) => run_frame(cli, emit, &path, cmd),
        Command::Psd(PsdCmd::Import { input }) => {
            anyhow::ensure!(cli.server.is_none(), "psd import는 디스크 모드 전용");
            anyhow::ensure!(!path.exists(), "이미 문서가 존재: {}", cli.doc.display());
            let bytes = std::fs::read(input).context("PSD 읽기")?;
            let doc = dcli_psd::import_psd(&bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
            if !cli.dry_run {
                path.save(&doc)?;
            }
            emit.ok_target(
                &format!(
                    "PSD import: {} → {} (레이어 {}개)",
                    input.display(),
                    cli.doc.display(),
                    doc.node_count()
                ),
                cli.dry_run,
                // 항상 디스크에 쓴다 — 데몬이 같은 이름을 열고 있어도 live로 표기하지 않는다.
                if cli.dry_run { None } else { Some("disk") },
            );
            Ok(())
        }
        Command::Psd(PsdCmd::Export { out }) => {
            anyhow::ensure!(cli.server.is_none(), "psd export는 디스크 모드 전용");
            let doc = path.load()?;
            let bytes = dcli_psd::export_psd(&doc);
            if !cli.dry_run {
                std::fs::write(out, &bytes).context("PSD 쓰기")?;
            }
            emit.ok_target(
                &format!("PSD export: {} ({} bytes)", out.display(), bytes.len()),
                cli.dry_run,
                // 디스크 문서를 읽어 디스크에 쓴다 — live 라벨 금지(스테일 스냅샷 오인 방지).
                if cli.dry_run { None } else { Some("disk") },
            );
            Ok(())
        }
        Command::Font(FontCmd::List) => {
            let mut names = vec![dcli_raster::text::DEFAULT_FONT.to_string()];
            names.extend(dcli_raster::sysfonts::scan().iter().map(|f| f.name.clone()));
            if cli.json {
                emit.raw_json_or(&serde_json::json!({ "fonts": names }), "fonts");
            } else {
                for n in &names {
                    println!("{n}");
                }
            }
            Ok(())
        }
        Command::Undo => {
            let srv = server_of(cli).ok_or_else(|| {
                anyhow::anyhow!("undo는 --server 모드 전용(디스크 모드는 세션 히스토리 없음)")
            })?;
            let changed = srv.undo()?;
            emit.ok_target(
                if changed {
                    "되돌림"
                } else {
                    "되돌릴 항목 없음"
                },
                false,
                Some("live"),
            );
            Ok(())
        }
        Command::Redo => {
            let srv = server_of(cli).ok_or_else(|| {
                anyhow::anyhow!("redo는 --server 모드 전용(디스크 모드는 세션 히스토리 없음)")
            })?;
            let changed = srv.redo()?;
            emit.ok_target(
                if changed {
                    "다시 적용"
                } else {
                    "다시 적용할 항목 없음"
                },
                false,
                Some("live"),
            );
            Ok(())
        }
    }
}

/// 현재 frame 목록(디스크: doc.frames, 서버: state.frames)을 FrameDto로.
fn current_frames(cli: &Cli, path: &DocPath) -> Result<Vec<dispatch::FrameDto>> {
    if let Some(srv) = server_of(cli) {
        let st = srv.state()?;
        let arr = st["frames"].as_array().cloned().unwrap_or_default();
        return Ok(arr
            .iter()
            .map(|f| dispatch::FrameDto {
                id: f["id"].as_u64().unwrap_or(0),
                name: f["name"].as_str().unwrap_or("").to_string(),
                x: f["x"].as_i64().unwrap_or(0) as i32,
                y: f["y"].as_i64().unwrap_or(0) as i32,
                w: f["w"].as_u64().unwrap_or(0) as u32,
                h: f["h"].as_u64().unwrap_or(0) as u32,
            })
            .collect());
    }
    let doc = path.load()?;
    Ok(doc
        .frames
        .iter()
        .map(|f| dispatch::FrameDto {
            id: f.id,
            name: f.name.clone(),
            x: f.x,
            y: f.y,
            w: f.w,
            h: f.h,
        })
        .collect())
}

fn run_frame(cli: &Cli, emit: &Emitter, path: &DocPath, cmd: &FrameCmd) -> Result<()> {
    match cmd {
        FrameCmd::Add { name, x, y, w, h } => {
            let mut frames = current_frames(cli, path)?;
            anyhow::ensure!(
                !frames.iter().any(|f| &f.name == name),
                "frame 이름 중복: {name}"
            );
            let id = frames.iter().map(|f| f.id).max().map_or(0, |m| m + 1);
            frames.push(dispatch::FrameDto {
                id,
                name: name.clone(),
                x: *x,
                y: *y,
                w: *w,
                h: *h,
            });
            apply_one(cli, path, Action::SetFrames { frames })?;
            emit.ok_target(
                &format!("frame 추가: \"{name}\" ({x},{y} {w}x{h})"),
                cli.dry_run,
                write_target(cli),
            );
            Ok(())
        }
        FrameCmd::List => {
            let frames = current_frames(cli, path)?;
            if cli.json {
                println!("{}", serde_json::to_string(&frames)?);
            } else if frames.is_empty() {
                println!("frame 없음 — dx frame add <이름> <x> <y> <w> <h>");
            } else {
                for f in &frames {
                    println!(
                        "  [{}] \"{}\" ({},{}) {}x{}",
                        f.id, f.name, f.x, f.y, f.w, f.h
                    );
                }
            }
            Ok(())
        }
        FrameCmd::Set { name, x, y, w, h, rename } => {
            let mut frames = current_frames(cli, path)?;
            let target = frames
                .iter_mut()
                .find(|f| &f.name == name || f.id.to_string() == *name)
                .ok_or_else(|| anyhow::anyhow!("frame 없음: {name}"))?;
            if let Some(v) = x { target.x = *v; }
            if let Some(v) = y { target.y = *v; }
            if let Some(v) = w { target.w = (*v).max(1); }
            if let Some(v) = h { target.h = (*v).max(1); }
            if let Some(n) = rename { target.name = n.clone(); }
            let label = format!("frame 수정: {} ({},{} {}x{})", target.name, target.x, target.y, target.w, target.h);
            apply_one(cli, path, Action::SetFrames { frames })?;
            emit.ok_target(&label, cli.dry_run, write_target(cli));
            Ok(())
        }
        FrameCmd::Remove { name } => {
            let mut frames = current_frames(cli, path)?;
            let before = frames.len();
            frames.retain(|f| &f.name != name && f.id.to_string() != *name);
            anyhow::ensure!(frames.len() < before, "frame 없음: {name}");
            apply_one(cli, path, Action::SetFrames { frames })?;
            emit.ok_target(
                &format!("frame 제거: {name}"),
                cli.dry_run,
                write_target(cli),
            );
            Ok(())
        }
    }
}

fn run_layer(cli: &Cli, emit: &Emitter, path: &DocPath, cmd: &LayerCmd) -> Result<()> {
    match cmd {
        LayerCmd::Add {
            name,
            image,
            fill,
            gradient,
            gradient_angle,
            gradient_radial,
            index,
        } => {
            // CLI 인자를 dispatch PixelSource로 변환.
            let mut fill_meta: Option<String> = None;
            let source = if let Some(img) = image {
                PixelSource::PngPath { path: img.clone() }
            } else if let Some(gspec) = gradient {
                // 문서 전체 그라데이션 — 각도를 문서 px 축으로 변환.
                let g = parse_gradient(gspec, *gradient_angle, *gradient_radial)?;
                let dims = if let Some(srv) = server_of(cli) {
                    let st = srv.state()?;
                    (
                        st["doc"]["w"].as_u64().unwrap_or(512) as f32,
                        st["doc"]["h"].as_u64().unwrap_or(512) as f32,
                    )
                } else {
                    let d = path.load()?;
                    (d.width as f32, d.height as f32)
                };
                let stops = g.stops.clone();
                if *gradient_radial {
                    PixelSource::RadialGradient {
                        cx: dims.0 * 0.5,
                        cy: dims.1 * 0.5,
                        radius: (dims.0.max(dims.1)) * 0.5,
                        stops,
                    }
                } else {
                    PixelSource::LinearGradient {
                        x0: g.x0 * dims.0,
                        y0: g.y0 * dims.1,
                        x1: g.x1 * dims.0,
                        y1: g.y1 * dims.1,
                        stops,
                    }
                }
            } else if let Some(f) = fill {
                // 단색 레이어는 벡터 rect meta를 함께 저장 — 뷰 합성이 전 배율 재래스터.
                let rgba = parse_rgba(f)?;
                let doc_for_dims = path.load().ok();
                if let Some(d) = &doc_for_dims {
                    let item = serde_json::json!({
                        "shape": "rect", "x": 0, "y": 0, "w": d.width, "h": d.height, "rgba": rgba,
                    });
                    fill_meta = Some(
                        serde_json::json!({
                            "type": "shape", "shape": "rect", "item": item,
                            "fill": rgba, "rgba": rgba, "stroke": null, "strokeWidth": 0,
                        })
                        .to_string(),
                    );
                }
                PixelSource::Fill { rgba }
            } else {
                PixelSource::Transparent
            };
            let mut actions = vec![Action::AddPaintLayer {
                name: name.clone(),
                source,
                index: *index,
                bind: Some("new".into()),
            }];
            if let Some(meta) = fill_meta {
                actions.push(Action::SetProps {
                    id: NodeRef::Bind("new".into()),
                    patch: PropPatch {
                        meta: Some(meta),
                        ..Default::default()
                    },
                });
            }
            // 공통 경로(디스크/서버). 발급된 id는 BatchResult.bindings에서 읽는다.
            let res = apply_actions(cli, path, actions)?;
            if cli.dry_run {
                emit.ok_target(&format!("레이어 추가(dry-run): \"{name}\""), true, None);
            } else {
                let b = &res.bindings["new"];
                emit.layer_added_target(
                    NodeId(b.node),
                    name,
                    dcli_tile::SurfaceId(b.surface.unwrap()),
                    false,
                    write_target(cli),
                );
            }
            Ok(())
        }
        LayerCmd::Style {
            id,
            fill,
            no_fill,
            gradient,
            gradient_angle,
            gradient_radial,
            stroke,
            stroke_width,
            no_stroke,
            radius,
            shadow,
            no_shadow,
        } => {
            let fill_rgba = fill.as_deref().map(parse_rgba).transpose()?;
            let grad = gradient
                .as_deref()
                .map(|g| parse_gradient(g, *gradient_angle, *gradient_radial))
                .transpose()?;
            let stroke_rgba = stroke.as_deref().map(parse_rgba).transpose()?;
            let shadow_v = shadow.as_deref().map(parse_shadow).transpose()?;
            restyle_layer(cli, &path, *id, |m| {
                anyhow::ensure!(
                    m["type"] == "shape" || m["type"] == "brush",
                    "layer style은 도형/브러시 레이어 전용(텍스트는 layer text)"
                );
                if *no_fill {
                    m["noFill"] = serde_json::json!(true);
                }
                if let Some(c) = fill_rgba {
                    m["noFill"] = serde_json::json!(false);
                    m["item"]["rgba"] = serde_json::json!(c);
                    m["fill"] = serde_json::json!(c);
                    m["rgba"] = serde_json::json!(c);
                    if let Some(o) = m["item"].as_object_mut() {
                        o.remove("gradient");
                    }
                }
                if let Some(g) = grad {
                    m["noFill"] = serde_json::json!(false);
                    m["item"]["gradient"] = serde_json::to_value(g)?;
                }
                if *no_stroke {
                    m["stroke"] = serde_json::Value::Null;
                    m["strokeWidth"] = serde_json::json!(0);
                }
                if let Some(c) = stroke_rgba {
                    m["stroke"] = serde_json::json!(c);
                    if m["strokeWidth"].as_f64().unwrap_or(0.0) <= 0.0 {
                        m["strokeWidth"] = serde_json::json!(4);
                    }
                }
                if let Some(w) = stroke_width {
                    m["strokeWidth"] = serde_json::json!(w.max(0.0));
                    if *w <= 0.0 {
                        m["stroke"] = serde_json::Value::Null;
                    }
                }
                if let Some(r) = radius {
                    let r = r.max(0.0);
                    m["radius"] = serde_json::json!(r);
                    let kind = m["item"]["shape"].as_str().unwrap_or("").to_string();
                    if kind == "rect" || kind == "rounded_rect" {
                        if r > 0.0 {
                            m["item"]["shape"] = serde_json::json!("rounded_rect");
                            m["item"]["radius"] = serde_json::json!(r);
                        } else {
                            m["item"]["shape"] = serde_json::json!("rect");
                            if let Some(o) = m["item"].as_object_mut() {
                                o.remove("radius");
                            }
                        }
                    }
                }
                if *no_shadow {
                    if let Some(o) = m.as_object_mut() {
                        o.remove("shadow");
                    }
                }
                if let Some(sh) = shadow_v {
                    m["shadow"] = sh;
                }
                Ok(())
            })?;
            emit.ok_target(&format!("레이어 스타일: n{id}"), cli.dry_run, write_target(cli));
            Ok(())
        }
        LayerCmd::Text {
            id,
            text,
            size,
            color,
            font,
            bg,
            no_bg,
            bg_radius,
            bg_pad,
        } => {
            if let Some(f) = font.as_deref().filter(|f| !f.is_empty()) {
                anyhow::ensure!(
                    dcli_raster::sysfonts::ensure(f),
                    "글꼴을 찾을 수 없음: {f} (dx font list로 확인)"
                );
            }
            let color_rgba = color.as_deref().map(parse_rgba).transpose()?;
            let bg_rgba = bg.as_deref().map(parse_rgba).transpose()?;
            restyle_layer(cli, &path, *id, |m| {
                anyhow::ensure!(m["type"] == "text", "layer text는 텍스트 레이어 전용");
                if let Some(t) = text {
                    m["text"] = serde_json::json!(t.replace("\\n", "\n"));
                }
                if let Some(sz) = size {
                    m["size"] = serde_json::json!(sz.clamp(6.0, 400.0));
                }
                if let Some(c) = color_rgba {
                    m["rgba"] = serde_json::json!(c);
                }
                if let Some(f) = font {
                    if f.is_empty() {
                        if let Some(o) = m.as_object_mut() {
                            o.remove("font");
                        }
                    } else {
                        m["font"] = serde_json::json!(f);
                    }
                }
                if *no_bg {
                    if let Some(o) = m.as_object_mut() {
                        o.remove("bg");
                    }
                }
                if let Some(c) = bg_rgba {
                    m["bg"] = serde_json::json!({ "rgba": c });
                }
                if m.get("bg").map(|b| !b.is_null()).unwrap_or(false) {
                    if let Some(r) = bg_radius {
                        m["bg"]["radius"] = serde_json::json!(r.max(0.0));
                    }
                    if let Some(pd) = bg_pad {
                        m["bg"]["padX"] = serde_json::json!(pd.max(0.0));
                        m["bg"]["padY"] = serde_json::json!((pd * 0.63).max(0.0));
                    }
                }
                Ok(())
            })?;
            emit.ok_target(&format!("텍스트 편집: n{id}"), cli.dry_run, write_target(cli));
            Ok(())
        }
        LayerCmd::List => {
            if let Some(srv) = server_of(cli) {
                let st = srv.state()?;
                emit.raw_json_or(&st["layers"], "레이어 (server)");
                return Ok(());
            }
            let doc = path.load()?;
            emit.layer_list(&doc);
            Ok(())
        }
        LayerCmd::Get { id } => {
            if let Some(srv) = server_of(cli) {
                // 데몬 state에서 해당 id 레이어를 찾아 출력.
                let st = srv.state()?;
                let found = st["layers"]
                    .as_array()
                    .and_then(|a| a.iter().find(|l| l["id"].as_u64() == Some(*id)))
                    .cloned();
                match found {
                    Some(l) => emit.raw_json_or(&l, "레이어 (server)"),
                    None => anyhow::bail!("레이어 없음: n{id}"),
                }
                return Ok(());
            }
            let doc = path.load()?;
            let node = doc
                .get(NodeId(*id))
                .ok_or_else(|| anyhow::anyhow!("레이어 없음: n{id}"))?;
            emit.layer_get(&doc, node);
            Ok(())
        }
        LayerCmd::Set {
            id,
            opacity,
            visible,
            name,
            x,
            y,
            scale_x,
            scale_y,
            rotation,
            meta,
        } => {
            // --x/--y, --scale-x/--scale-y는 clap requires로 항상 쌍.
            let offset = x.zip(*y);
            let scale = scale_x.zip(*scale_y);
            let patch = PropPatch {
                name: name.clone(),
                visible: *visible,
                opacity: *opacity,
                offset,
                scale,
                rotation: *rotation,
                meta: meta.clone(),
            };
            apply_one(
                cli,
                path,
                Action::SetProps {
                    id: NodeRef::Node(*id),
                    patch,
                },
            )?;
            emit.ok_target(
                &format!("레이어 속성 변경: n{id}"),
                cli.dry_run,
                write_target(cli),
            );
            Ok(())
        }
        LayerCmd::Move { id, to } => {
            apply_one(
                cli,
                path,
                Action::MoveLayer {
                    id: NodeRef::Node(*id),
                    to: *to,
                },
            )?;
            emit.ok_target(
                &format!("레이어 이동: n{id} → idx {to}"),
                cli.dry_run,
                write_target(cli),
            );
            Ok(())
        }
        LayerCmd::Delete { id } => {
            apply_one(
                cli,
                path,
                Action::DeleteLayer {
                    id: NodeRef::Node(*id),
                },
            )?;
            emit.ok_target(
                &format!("레이어 삭제: n{id}"),
                cli.dry_run,
                write_target(cli),
            );
            Ok(())
        }
        LayerCmd::Group { ids, name } => {
            let refs: Vec<NodeRef> = ids.iter().map(|i| NodeRef::Node(*i)).collect();
            apply_one(
                cli,
                path,
                Action::GroupLayers {
                    ids: refs,
                    name: name.clone(),
                    bind: None,
                },
            )?;
            emit.ok_target(
                &format!("그룹 생성: \"{name}\" ({}개)", ids.len()),
                cli.dry_run,
                write_target(cli),
            );
            Ok(())
        }
        LayerCmd::Ungroup { id } => {
            apply_one(
                cli,
                path,
                Action::Ungroup {
                    id: NodeRef::Node(*id),
                },
            )?;
            emit.ok_target(&format!("그룹 해제: n{id}"), cli.dry_run, write_target(cli));
            Ok(())
        }
        LayerCmd::Duplicate { id } => {
            let res = apply_one(
                cli,
                path,
                Action::DuplicateLayer {
                    id: NodeRef::Node(*id),
                    bind: Some("copy".into()),
                },
            )?;
            let new_id = res.bindings.get("copy").map(|b| b.node);
            emit.ok_target(
                &match new_id {
                    Some(n) => format!("레이어 복제: n{id} → n{n}"),
                    None => format!("레이어 복제: n{id}"),
                },
                cli.dry_run,
                write_target(cli),
            );
            Ok(())
        }
    }
}
