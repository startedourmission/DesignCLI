//! egui-wgpu 네이티브 프리뷰 창 (Phase 0의 3번째 렌더 경로).
//!
//! 핵심: 동일 코어(dcli-raster CPU 정본)로 합성한 표면을 네이티브 창에 띄운다.
//! 코어는 egui/eframe에 의존하지 않는다 — 이 의존은 example crate에만 있다
//! (코어 UI-무의존 불변식, implementation-plan.md).

use dcli_color::BitDepth;
use dcli_model::fixtures::spike_scene;

struct PreviewApp {
    tex: Option<egui::TextureHandle>,
    depth: BitDepth,
}

impl PreviewApp {
    fn new(depth: BitDepth) -> Self {
        Self { tex: None, depth }
    }

    fn composite_image(&self) -> egui::ColorImage {
        let doc = spike_scene(self.depth);
        let rgba = dcli_raster::composite(&doc).to_srgb8_rgba();
        egui::ColorImage::from_rgba_unmultiplied(
            [doc.width as usize, doc.height as usize],
            &rgba,
        )
    }
}

impl eframe::App for PreviewApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.tex.is_none() {
            let img = self.composite_image();
            self.tex = Some(ctx.load_texture("composite", img, egui::TextureOptions::NEAREST));
        }
        let tex = self.tex.as_ref().unwrap();
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(format!(
                "DesignCLI Phase 0 — {} 합성 (CPU 정본)",
                match self.depth {
                    BitDepth::F32 => "linear(F32)",
                    _ => "gamma(U8)",
                }
            ));
            ui.label("배경: 좌=주황/우=청록 · Multiply 회색 · Screen 파랑");
            let size = tex.size_vec2() * 4.0; // 확대 표시
            ui.add(egui::Image::new(tex).fit_to_exact_size(size));
        });
    }
}

/// macOS 시스템 한글 폰트를 egui에 등록한다(없으면 기본 폰트 유지 → 한글은 □).
fn install_korean_font(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
        "/System/Library/Fonts/Supplemental/AppleGothic.ttf",
        "/System/Library/Fonts/STHeiti Light.ttc",
    ];
    let Some(bytes) = CANDIDATES
        .iter()
        .find_map(|p| std::fs::read(p).ok())
    else {
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("kr".to_owned(), egui::FontData::from_owned(bytes));
    // 기본 proportional/monospace 앞에 한글 폰트를 우선 배치.
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().insert(0, "kr".to_owned());
    }
    ctx.set_fonts(fonts);
}

pub fn run_preview(depth: BitDepth) -> anyhow::Result<()> {
    let opts = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "DesignCLI parity preview",
        opts,
        Box::new(move |cc| {
            install_korean_font(&cc.egui_ctx);
            Ok(Box::new(PreviewApp::new(depth)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe 실행 실패: {e}"))
}
