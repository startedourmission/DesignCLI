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
#[command(name = "dx", version, about = "DesignCLI — CLI로 조작하는 이미지 에디터")]
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
    /// 마지막 편집을 되돌린다(--server 모드 전용 — 디스크 모드는 세션 히스토리 없음).
    Undo,
    /// 되돌린 편집을 다시 적용한다(--server 모드 전용).
    Redo,
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
        #[arg(long, requires = "y")]
        x: Option<i32>,
        /// 캔버스 Y 평행이동(절대 offset, 픽셀). --x와 함께.
        #[arg(long, requires = "x")]
        y: Option<i32>,
    },
    /// 레이어를 새 순서 인덱스로 이동.
    Move { id: u64, to: usize },
    /// 레이어 삭제.
    Delete { id: u64 },
}

#[derive(Subcommand)]
enum BlendCmd {
    /// 레이어 블렌드 모드 변경: normal|multiply|screen.
    Set { id: u64, mode: String },
}

#[derive(Subcommand)]
enum ExportCmd {
    /// 합성 결과를 PNG로 저장.
    Png { out: PathBuf },
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
        other => anyhow::bail!("알 수 없는 블렌드 모드: {other} (normal|multiply|screen)"),
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
        DrawCmd::Rect { x, y, w, h, color, name } => (
            Shape::Rect { x: *x, y: *y, w: *w, h: *h, rgba: parse_rgba(color)? },
            name.clone(),
        ),
        DrawCmd::Ellipse { cx, cy, rx, ry, color, name } => (
            Shape::Ellipse { cx: *cx, cy: *cy, rx: *rx, ry: *ry, rgba: parse_rgba(color)? },
            name.clone(),
        ),
        DrawCmd::Line { x0, y0, x1, y1, width, color, name } => (
            Shape::Line { x0: *x0, y0: *y0, x1: *x1, y1: *y1, width: *width, rgba: parse_rgba(color)? },
            name.clone(),
        ),
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
                res.issues.first().map(|i| i.message.clone()).unwrap_or_else(|| "적용 실패".into())
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
            issue.map(|i| i.message.clone()).unwrap_or_else(|| "적용 실패".into())
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
            anyhow::ensure!(!path.exists() || cli.dry_run, "이미 문서가 존재: {}", cli.doc.display());
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
        Command::Layer(cmd) => run_layer(cli, emit, &path, cmd),
        Command::Blend(BlendCmd::Set { id, mode }) => {
            let mode = parse_blend(mode)?;
            apply_one(cli, &path, Action::SetBlend { id: NodeRef::Node(*id), mode })?;
            emit.ok(&format!("블렌드 설정: n{id} = {mode:?}"), cli.dry_run);
            Ok(())
        }
        Command::Draw(cmd) => {
            let (shape, name) = draw_to_shape(cmd)?;
            // 한 도형을 새 레이어로 그린다(layer add의 Shapes source).
            let action = Action::AddPaintLayer {
                name: name.clone(),
                source: PixelSource::Shapes { items: vec![shape] },
                index: None,
                bind: Some("new".into()),
            };
            apply_one(cli, &path, action)?;
            emit.ok(&format!("도형 그림: \"{name}\""), cli.dry_run);
            Ok(())
        }
        Command::Export(ExportCmd::Png { out }) => {
            anyhow::ensure!(
                cli.server.is_none(),
                "export는 --server 모드 미지원(프로토타입) — 웹 UI의 🖼 PNG 버튼을 쓰세요"
            );
            let doc = path.load()?;
            let surface = dcli_raster::composite(&doc);
            if !cli.dry_run {
                storage::export_png(out, &surface)?;
            }
            emit.exported(out, surface.width(), surface.height(), cli.dry_run);
            Ok(())
        }
        Command::Undo => {
            let srv = server_of(cli).ok_or_else(|| {
                anyhow::anyhow!("undo는 --server 모드 전용(디스크 모드는 세션 히스토리 없음)")
            })?;
            let changed = srv.undo()?;
            emit.ok(if changed { "되돌림" } else { "되돌릴 항목 없음" }, false);
            Ok(())
        }
        Command::Redo => {
            let srv = server_of(cli).ok_or_else(|| {
                anyhow::anyhow!("redo는 --server 모드 전용(디스크 모드는 세션 히스토리 없음)")
            })?;
            let changed = srv.redo()?;
            emit.ok(if changed { "다시 적용" } else { "다시 적용할 항목 없음" }, false);
            Ok(())
        }
    }
}

fn run_layer(cli: &Cli, emit: &Emitter, path: &DocPath, cmd: &LayerCmd) -> Result<()> {
    match cmd {
        LayerCmd::Add { name, image, fill, index } => {
            // CLI 인자를 dispatch PixelSource로 변환.
            let source = if let Some(img) = image {
                PixelSource::PngPath { path: img.clone() }
            } else if let Some(f) = fill {
                PixelSource::Fill { rgba: parse_rgba(f)? }
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
                emit.layer_added(NodeId(b.node), name, dcli_tile::SurfaceId(b.surface.unwrap()), false);
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
            let node = doc.get(NodeId(*id)).ok_or_else(|| anyhow::anyhow!("레이어 없음: n{id}"))?;
            emit.layer_get(node);
            Ok(())
        }
        LayerCmd::Set { id, opacity, visible, name, x, y } => {
            // --x/--y는 clap requires로 항상 쌍 → 둘 다 Some일 때만 offset.
            let offset = x.zip(*y);
            let patch = PropPatch {
                name: name.clone(),
                visible: *visible,
                opacity: *opacity,
                offset,
            };
            apply_one(cli, path, Action::SetProps { id: NodeRef::Node(*id), patch })?;
            emit.ok(&format!("레이어 속성 변경: n{id}"), cli.dry_run);
            Ok(())
        }
        LayerCmd::Move { id, to } => {
            apply_one(cli, path, Action::MoveLayer { id: NodeRef::Node(*id), to: *to })?;
            emit.ok(&format!("레이어 이동: n{id} → idx {to}"), cli.dry_run);
            Ok(())
        }
        LayerCmd::Delete { id } => {
            apply_one(cli, path, Action::DeleteLayer { id: NodeRef::Node(*id) })?;
            emit.ok(&format!("레이어 삭제: n{id}"), cli.dry_run);
            Ok(())
        }
    }
}
