//! 네이티브 스튜디오 앱 상태 + UI.
//!
//! ★불변식★ 앱은 자체 문서 상태를 두지 않는다. 진실의 원천은 코어 `History`이며,
//! 모든 편집은 `Op`을 통해서만 일어난다(이중 상태관리 금지, implementation-plan.md).
//! UI는 코어 op 스택에서 파생된 뷰일 뿐이고, 캔버스는 CPU 정본(dcli-raster) 재합성을
//! egui 텍스처로 보여준다 — 셸 프리뷰가 export 정본과 자동으로 일치한다.

use dcli_cli::storage::{self, DocPath};
use dcli_color::{BitDepth, LinearPremul};
use dcli_model::{BlendMode, Document, History, NodeId, NodeProps, Op};
use dcli_tile::Surface;
use std::path::PathBuf;

pub struct StudioApp {
    /// 진실의 원천: 코어 문서 + event-sourced 히스토리.
    hist: History,
    /// 현재 선택된 노드.
    selected: Option<NodeId>,
    /// 합성 캐시 텍스처(재합성 필요 시 무효화).
    tex: Option<egui::TextureHandle>,
    /// 문서가 바뀌어 재합성이 필요한가.
    dirty: bool,
    /// 저장 경로(있으면 export/save에 사용).
    doc_path: Option<PathBuf>,
    /// 마지막 상태 메시지.
    status: String,
}

impl StudioApp {
    pub fn new(doc: Document, doc_path: Option<PathBuf>) -> Self {
        Self {
            hist: History::new(doc),
            selected: None,
            tex: None,
            dirty: true,
            doc_path,
            status: "준비됨".to_owned(),
        }
    }

    /// op을 코어에 적용하고 재합성을 예약한다. 모든 편집의 단일 통로.
    fn apply(&mut self, op: Op) {
        match self.hist.apply(op) {
            Ok(()) => self.dirty = true,
            Err(e) => self.status = format!("op 실패: {e}"),
        }
    }

    fn recomposite(&mut self, ctx: &egui::Context) {
        let surface = dcli_raster::composite(&self.hist.doc);
        let img = egui::ColorImage::from_rgba_unmultiplied(
            [surface.width() as usize, surface.height() as usize],
            &surface.to_srgb8_rgba(),
        );
        match &mut self.tex {
            Some(t) => t.set(img, egui::TextureOptions::NEAREST),
            None => self.tex = Some(ctx.load_texture("composite", img, egui::TextureOptions::NEAREST)),
        }
        self.dirty = false;
    }

    fn add_solid_layer(&mut self) {
        let (w, h) = (self.hist.doc.width, self.hist.doc.height);
        // 반투명 회색 새 레이어(데모용 채움).
        let surface = Surface::filled(w, h, LinearPremul::from_srgb8_straight(150, 150, 150, 180));
        let sid = self.hist.doc.add_surface(surface);
        self.apply(Op::AddPaintLayer { name: "layer".into(), surface: sid, index: None, forced_id: None });
        self.selected = self.hist.doc.order().last().copied();
    }

    fn save(&mut self) {
        let Some(path) = &self.doc_path else {
            self.status = "저장 경로 없음(--doc로 실행)".into();
            return;
        };
        match DocPath::new(path.clone()).save(&self.hist.doc) {
            Ok(()) => self.status = format!("저장됨: {}", path.display()),
            Err(e) => self.status = format!("저장 실패: {e}"),
        }
    }

    fn export_png(&mut self) {
        let out = self
            .doc_path
            .as_ref()
            .map(|p| p.with_extension("png"))
            .unwrap_or_else(|| PathBuf::from("export.png"));
        let surface = dcli_raster::composite(&self.hist.doc);
        match storage::export_png(&out, &surface) {
            Ok(()) => self.status = format!("export: {}", out.display()),
            Err(e) => self.status = format!("export 실패: {e}"),
        }
    }
}

impl eframe::App for StudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.dirty || self.tex.is_none() {
            self.recomposite(ctx);
        }

        self.top_bar(ctx);
        self.layers_panel(ctx);
        self.canvas(ctx);
        self.status_bar(ctx);
    }
}

impl StudioApp {
    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("DesignCLI Studio");
                ui.separator();
                if ui.button("＋ 레이어").clicked() {
                    self.add_solid_layer();
                }
                ui.add_enabled(self.hist.can_undo(), egui::Button::new("↶ Undo")).clicked().then(|| {
                    let _ = self.hist.undo();
                    self.dirty = true;
                    self.fix_selection();
                });
                ui.add_enabled(self.hist.can_redo(), egui::Button::new("↷ Redo")).clicked().then(|| {
                    let _ = self.hist.redo();
                    self.dirty = true;
                    self.fix_selection();
                });
                ui.separator();
                if ui.button("💾 저장").clicked() {
                    self.save();
                }
                if ui.button("🖼 PNG export").clicked() {
                    self.export_png();
                }
            });
        });
    }

    /// 선택 노드가 사라졌으면(undo 등) 선택을 정리.
    fn fix_selection(&mut self) {
        if let Some(id) = self.selected {
            if self.hist.doc.get(id).is_none() {
                self.selected = self.hist.doc.order().last().copied();
            }
        }
    }

    fn layers_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("layers").default_width(260.0).show(ctx, |ui| {
            ui.heading("레이어");
            ui.label("(위가 앞쪽 = 위에 그려짐)");
            ui.separator();

            // top-to-bottom 표시(UI 관습) → order는 bottom-to-top이라 역순.
            let ids: Vec<NodeId> = self.hist.doc.order().iter().rev().copied().collect();
            for id in ids {
                self.layer_row(ui, id);
            }
        });
    }

    fn layer_row(&mut self, ui: &mut egui::Ui, id: NodeId) {
        let Some(node) = self.hist.doc.get(id) else { return };
        // 현재 속성 스냅샷(아래에서 op으로만 변경).
        let mut props = NodeProps::of(node);
        let is_sel = self.selected == Some(id);

        let frame = egui::Frame::group(ui.style()).fill(if is_sel {
            ui.visuals().selection.bg_fill
        } else {
            ui.visuals().faint_bg_color
        });

        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(is_sel, format!("n{} {}", id.0, props.name)).clicked() {
                    self.selected = Some(id);
                }
            });

            let mut changed = false;
            ui.horizontal(|ui| {
                if ui.checkbox(&mut props.visible, "보임").changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("불투명");
                if ui.add(egui::Slider::new(&mut props.opacity, 0.0..=1.0)).changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("블렌드");
                egui::ComboBox::from_id_source(("blend", id.0))
                    .selected_text(format!("{:?}", props.blend))
                    .show_ui(ui, |ui| {
                        for m in [BlendMode::Normal, BlendMode::Multiply, BlendMode::Screen] {
                            if ui.selectable_value(&mut props.blend, m, format!("{m:?}")).changed() {
                                changed = true;
                            }
                        }
                    });
            });
            ui.horizontal(|ui| {
                // 순서 이동·삭제.
                if ui.small_button("▲").clicked() {
                    self.move_relative(id, 1);
                }
                if ui.small_button("▼").clicked() {
                    self.move_relative(id, -1);
                }
                if ui.small_button("🗑").clicked() {
                    self.apply(Op::DeleteLayer { id });
                    if self.selected == Some(id) {
                        self.selected = self.hist.doc.order().last().copied();
                    }
                }
            });

            // 슬라이더/체크/콤보가 바뀌었으면 op으로 적용(직접 노드 변경 금지).
            if changed {
                self.apply(Op::SetProps { id, props });
            }
        });
    }

    /// 순서 인덱스를 delta만큼 이동(+1=위로, -1=아래로).
    fn move_relative(&mut self, id: NodeId, delta: i32) {
        let Some(cur) = self.hist.doc.order().iter().position(|&n| n == id) else { return };
        let len = self.hist.doc.order().len() as i32;
        let to = (cur as i32 + delta).clamp(0, len - 1) as usize;
        if to != cur {
            self.apply(Op::MoveLayer { id, to });
        }
    }

    fn canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(tex) = &self.tex {
                let avail = ui.available_size();
                let sz = tex.size_vec2();
                // 가용 공간에 맞춰 정수배 확대(픽셀 선명).
                let scale = (avail.x / sz.x).min(avail.y / sz.y).max(1.0).floor();
                ui.centered_and_justified(|ui| {
                    ui.add(egui::Image::new(tex).fit_to_exact_size(sz * scale));
                });
            } else {
                ui.label("(빈 문서)");
            }
        });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let d = &self.hist.doc;
                ui.label(format!(
                    "{}x{} · {:?} · {:?} 합성 · 레이어 {}",
                    d.width, d.height, d.bit_depth, d.blend_space, d.node_count()
                ));
                ui.separator();
                ui.label(&self.status);
            });
        });
    }
}

/// 새 빈 문서(파일 경로 없이 시작할 때).
pub fn blank_document() -> Document {
    Document::new(256, 256, BitDepth::U8)
}
