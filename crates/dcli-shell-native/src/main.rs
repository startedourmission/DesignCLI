//! `dx-studio` — DesignCLI 네이티브 셸(winit+wgpu+egui).
//!
//! 동일 코어를 직링크한다. 모든 편집은 코어 op을 통과하므로 셸·CLI·export가
//! 같은 픽셀을 낸다. CLI 문서 폴더(.dxdoc)를 그대로 열고 저장한다.

mod app;

use anyhow::Result;
use app::{blank_document, StudioApp};
use dcli_cli::storage::DocPath;
use std::path::PathBuf;

fn parse_args() -> Option<PathBuf> {
    // 단순 인자: `dx-studio [문서폴더]` 또는 `--doc <폴더>`.
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--doc" => return args.next().map(PathBuf::from),
            s if !s.starts_with('-') => return Some(PathBuf::from(s)),
            _ => {}
        }
    }
    None
}

/// macOS 시스템 한글 폰트를 egui에 등록(없으면 기본 폰트 유지).
fn install_korean_font(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
        "/System/Library/Fonts/Supplemental/AppleGothic.ttf",
        "/System/Library/Fonts/STHeiti Light.ttc",
    ];
    let Some(bytes) = CANDIDATES.iter().find_map(|p| std::fs::read(p).ok()) else {
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("kr".to_owned(), egui::FontData::from_owned(bytes));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().insert(0, "kr".to_owned());
    }
    ctx.set_fonts(fonts);
}

fn main() -> Result<()> {
    let path = parse_args();

    // 경로가 주어졌고 문서가 있으면 로드, 없으면 빈 문서(첫 저장 시 그 경로에).
    let (doc, doc_path) = match &path {
        Some(p) if DocPath::new(p.clone()).exists() => (DocPath::new(p.clone()).load()?, Some(p.clone())),
        Some(p) => (blank_document(), Some(p.clone())),
        None => (blank_document(), None),
    };

    let opts = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "DesignCLI Studio",
        opts,
        Box::new(move |cc| {
            install_korean_font(&cc.egui_ctx);
            Ok(Box::new(StudioApp::new(doc, doc_path)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe 실행 실패: {e}"))
}
