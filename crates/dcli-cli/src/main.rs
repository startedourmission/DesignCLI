//! `dx` — DesignCLI 명령줄 인터페이스.
//!
//! CLI 서브커맨드는 코어 op과 1:1 대응한다(cli-agent-interface: CLI verb ≡ MCP tool).
//! 횡단 플래그: --doc(작업 대상 폴더), --json(stdout=데이터), --dry-run(적용될 변경만).
//!
//! 작업 흐름: 대부분의 명령은 문서 폴더를 load → op 적용 → save 한다. --dry-run이면
//! 적용 결과를 보여주되 save하지 않는다.

mod output;

use dcli_cli::dispatch::{self, Action, BatchResult, BlendModeDto, NodeRef, PixelSource, PropPatch};
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
    #[arg(long, global = true, default_value = "doc.dxdoc")]
    doc: PathBuf,

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
    /// 합성 결과를 파일로 export.
    #[command(subcommand)]
    Export(ExportCmd),
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
    /// 레이어 속성 변경(opacity/visible/name).
    Set {
        id: u64,
        #[arg(long)]
        opacity: Option<f32>,
        #[arg(long)]
        visible: Option<bool>,
        #[arg(long)]
        name: Option<String>,
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

/// 단발 Action을 문서에 적용한다(CLI 쓰기 공통 경로 = dispatch 1-op batch).
/// CLI와 MCP가 같은 엔진을 쓰도록 보장한다. 성공 시 BatchResult 반환.
fn apply_one(cli: &Cli, path: &DocPath, action: Action) -> Result<BatchResult> {
    let doc = path.load()?;
    let mut h = History::new(doc);
    let res = dispatch::apply_batch(&mut h, &[action], cli.dry_run);
    if !res.ok {
        // 단발 op 실패 → 첫 이슈를 에러로.
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

fn run(cli: &Cli, emit: &Emitter) -> Result<()> {
    let path = DocPath::new(cli.doc.clone());
    match &cli.command {
        Command::Doc(DocCmd::Create { w, h, depth }) => {
            let depth = parse_depth(depth)?;
            anyhow::ensure!(!path.exists() || cli.dry_run, "이미 문서가 존재: {}", cli.doc.display());
            let doc = Document::new(*w, *h, depth);
            if !cli.dry_run {
                path.save(&doc)?;
            }
            emit.doc_created(&cli.doc, &doc, cli.dry_run);
            Ok(())
        }
        Command::Doc(DocCmd::Info) => {
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
        Command::Export(ExportCmd::Png { out }) => {
            let doc = path.load()?;
            let surface = dcli_raster::composite(&doc);
            if !cli.dry_run {
                storage::export_png(out, &surface)?;
            }
            emit.exported(out, surface.width(), surface.height(), cli.dry_run);
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
            // layer add는 dry-run 아닐 때 발급된 id를 보고해야 하므로 직접 처리.
            let doc = path.load()?;
            let mut h = History::new(doc);
            let action = Action::AddPaintLayer {
                name: name.clone(),
                source,
                index: *index,
                bind: Some("new".into()),
            };
            let res = dispatch::apply_batch(&mut h, &[action], cli.dry_run);
            if !res.ok {
                anyhow::bail!(
                    "{}",
                    res.issues.first().map(|i| i.message.clone()).unwrap_or_else(|| "추가 실패".into())
                );
            }
            if cli.dry_run {
                emit.ok(&format!("레이어 추가(dry-run): \"{name}\""), true);
            } else {
                path.save(&h.doc)?;
                let b = &res.bindings["new"];
                emit.layer_added(NodeId(b.node), name, dcli_tile::SurfaceId(b.surface.unwrap()), false);
            }
            Ok(())
        }
        LayerCmd::List => {
            let doc = path.load()?;
            emit.layer_list(&doc);
            Ok(())
        }
        LayerCmd::Get { id } => {
            let doc = path.load()?;
            let node = doc.get(NodeId(*id)).ok_or_else(|| anyhow::anyhow!("레이어 없음: n{id}"))?;
            emit.layer_get(node);
            Ok(())
        }
        LayerCmd::Set { id, opacity, visible, name } => {
            let patch = PropPatch { name: name.clone(), visible: *visible, opacity: *opacity };
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
