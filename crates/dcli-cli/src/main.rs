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
    #[arg(long, global = true, default_value = "doc.dxdoc")]
    doc: PathBuf,

    /// 라이브 데몬(dx-daemon) URL. 지정 시 디스크 대신 데몬에 편집을 보낸다
    /// (웹 UI에 실시간 반영). 예: --server http://localhost:8137
    #[arg(long, global = true)]
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
    /// Frame 제거(이름 또는 id).
    Remove { name: String },
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
    /// 텍스트(한글/라틴, 번들 폰트): 좌상단 (x,y), 내용 TEXT. '\n' 줄바꿈.
    Text {
        x: f32,
        y: f32,
        text: String,
        #[arg(long, default_value_t = 24.0)]
        size: f32,
        #[arg(long, default_value = "0,0,0,255")]
        color: String,
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
        /// 삽입 위치(bottom-to-top 인덱스, 없으면 맨 위).
        #[arg(long)]
        index: Option<usize>,
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
            },
            name.clone(),
        ),
        DrawCmd::Text {
            x,
            y,
            text,
            size,
            color,
            name,
        } => (
            Shape::Text {
                x: *x,
                y: *y,
                text: text.clone(),
                size: *size,
                rgba: parse_rgba(color)?,
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

/// --server 모드면 데몬 문서 id를 만든다. id = --doc 경로의 파일명 stem
/// (예: "demo.dxdoc" → "demo", "demo" → "demo"). 디스크 모드면 None.
fn server_of(cli: &Cli) -> Option<Server> {
    let base = cli.server.as_ref()?;
    let id = cli
        .doc
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("doc")
        .to_string();
    Some(Server::new(base, &id))
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
                // 데몬에 문서 등록(이미 있으면 멱등). 데몬이 진실원.
                srv.ensure_doc(*w, *h, depth)?;
                let doc = Document::new(*w, *h, depth_bd);
                emit.doc_created(&cli.doc, &doc, false);
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
            emit.doc_created(&cli.doc, &doc, cli.dry_run);
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
            anyhow::ensure!(cli.server.is_none(), "doc compact는 디스크 모드 전용");
            let mut doc = path.load()?;
            let changed = dispatch::compact_text_surfaces(&mut doc);
            if changed > 0 && !cli.dry_run {
                path.save(&doc)?;
            }
            emit.ok(&format!("문서 압축: 텍스트 표면 {changed}개"), cli.dry_run);
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
            emit.ok(&format!("블렌드 설정: n{id} = {mode:?}"), cli.dry_run);
            Ok(())
        }
        Command::Draw(cmd) => {
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
            // 텍스트는 편집용 meta를 자동 저장(웹 더블클릭 편집과 호환).
            if let Shape::Text {
                x,
                y,
                text,
                size,
                rgba,
            } = &shape
            {
                let meta = serde_json::json!({
                    "type": "text", "x": x, "y": y, "text": text, "size": size, "rgba": rgba,
                })
                .to_string();
                actions.push(Action::SetProps {
                    id: NodeRef::Bind("new".into()),
                    patch: PropPatch {
                        meta: Some(meta),
                        ..Default::default()
                    },
                });
            }
            apply_actions(cli, &path, actions)?;
            emit.ok(&format!("도형 그림: \"{name}\""), cli.dry_run);
            Ok(())
        }
        Command::Export(ExportCmd::Png { out, frame, region }) => {
            // --server면 데몬이 합성·인코딩한 PNG를 받아 저장(디스크 모드와 동일 인코딩 경로).
            if let Some(srv) = server_of(cli) {
                let (w, h) = srv.export_png_with(out, frame.as_deref(), region.as_deref())?;
                emit.exported(out, w, h, false);
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
            emit.exported(out, surface.width(), surface.height(), cli.dry_run);
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
            emit.ok(
                &format!(
                    "PSD import: {} → {} (레이어 {}개)",
                    input.display(),
                    cli.doc.display(),
                    doc.node_count()
                ),
                cli.dry_run,
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
            emit.ok(
                &format!("PSD export: {} ({} bytes)", out.display(), bytes.len()),
                cli.dry_run,
            );
            Ok(())
        }
        Command::Undo => {
            let srv = server_of(cli).ok_or_else(|| {
                anyhow::anyhow!("undo는 --server 모드 전용(디스크 모드는 세션 히스토리 없음)")
            })?;
            let changed = srv.undo()?;
            emit.ok(
                if changed {
                    "되돌림"
                } else {
                    "되돌릴 항목 없음"
                },
                false,
            );
            Ok(())
        }
        Command::Redo => {
            let srv = server_of(cli).ok_or_else(|| {
                anyhow::anyhow!("redo는 --server 모드 전용(디스크 모드는 세션 히스토리 없음)")
            })?;
            let changed = srv.redo()?;
            emit.ok(
                if changed {
                    "다시 적용"
                } else {
                    "다시 적용할 항목 없음"
                },
                false,
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
            emit.ok(
                &format!("frame 추가: \"{name}\" ({x},{y} {w}x{h})"),
                cli.dry_run,
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
        FrameCmd::Remove { name } => {
            let mut frames = current_frames(cli, path)?;
            let before = frames.len();
            frames.retain(|f| &f.name != name && f.id.to_string() != *name);
            anyhow::ensure!(frames.len() < before, "frame 없음: {name}");
            apply_one(cli, path, Action::SetFrames { frames })?;
            emit.ok(&format!("frame 제거: {name}"), cli.dry_run);
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
            index,
        } => {
            // CLI 인자를 dispatch PixelSource로 변환.
            let source = if let Some(img) = image {
                PixelSource::PngPath { path: img.clone() }
            } else if let Some(f) = fill {
                PixelSource::Fill {
                    rgba: parse_rgba(f)?,
                }
            } else {
                PixelSource::Transparent
            };
            let action = Action::AddPaintLayer {
                name: name.clone(),
                source,
                index: *index,
                bind: Some("new".into()),
            };
            // 공통 경로(디스크/서버). 발급된 id는 BatchResult.bindings에서 읽는다.
            let res = apply_actions(cli, path, vec![action])?;
            if cli.dry_run {
                emit.ok(&format!("레이어 추가(dry-run): \"{name}\""), true);
            } else {
                let b = &res.bindings["new"];
                emit.layer_added(
                    NodeId(b.node),
                    name,
                    dcli_tile::SurfaceId(b.surface.unwrap()),
                    false,
                );
            }
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
            emit.layer_get(node);
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
            emit.ok(&format!("레이어 속성 변경: n{id}"), cli.dry_run);
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
            emit.ok(&format!("레이어 이동: n{id} → idx {to}"), cli.dry_run);
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
            emit.ok(&format!("레이어 삭제: n{id}"), cli.dry_run);
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
            emit.ok(
                &format!("그룹 생성: \"{name}\" ({}개)", ids.len()),
                cli.dry_run,
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
            emit.ok(&format!("그룹 해제: n{id}"), cli.dry_run);
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
            emit.ok(
                &match new_id {
                    Some(n) => format!("레이어 복제: n{id} → n{n}"),
                    None => format!("레이어 복제: n{id}"),
                },
                cli.dry_run,
            );
            Ok(())
        }
    }
}
