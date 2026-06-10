//! DesignCLI 웹 바인딩 — 코어 History+dispatch를 브라우저에 노출하는 얇은 어댑터.
//!
//! ★단일 진실원★ op는 Action JSON 문자열로 받아 dispatch::apply_batch에 직통하고,
//! 결과·레이어 목록은 dto/serde가 만든 JSON 문자열로 반환한다(JsValue 재모델링 없음).
//! 픽셀(합성 결과)은 straight-alpha sRGB8 RGBA → Uint8ClampedArray 복사본으로 넘긴다
//! (canvas ImageData가 요구하는 정확한 포맷, 메모리 detach 버그 클래스 회피).
//!
//! 코어가 wasm32로 빌드됨을 이미 검증했고, dcli-cli는 default-features=false(clap·
//! storage·std::fs 없음)로 의존하므로 브라우저 빌드가 깨끗하다.

use dcli_cli::dispatch::{self, Action};
use dcli_cli::{dto, dxpkg};
use dcli_color::BitDepth;
use dcli_model::{Document, History};
use wasm_bindgen::prelude::*;

/// 한 문서 편집 세션. History(undo 포함)를 소유한다.
#[wasm_bindgen]
pub struct Editor {
    hist: History,
    /// 합성 캐시(dirty일 때만 재합성).
    rgba: Vec<u8>,
    dirty: bool,
}

fn parse_depth(s: &str) -> Result<BitDepth, String> {
    match s {
        "u8" => Ok(BitDepth::U8),
        "u16" => Ok(BitDepth::U16),
        "f32" => Ok(BitDepth::F32),
        other => Err(format!("알 수 없는 비트깊이: {other} (u8|u16|f32)")),
    }
}

#[wasm_bindgen]
impl Editor {
    /// 새 문서. depth = "u8"(감마 합성) | "u16" | "f32"(리니어 합성).
    #[wasm_bindgen(constructor)]
    pub fn new(w: u32, h: u32, depth: &str) -> Result<Editor, JsError> {
        let depth = parse_depth(depth).map_err(|e| JsError::new(&e))?;
        let doc = Document::new(w, h, depth);
        Ok(Editor { hist: History::new(doc), rgba: Vec::new(), dirty: true })
    }

    /// Action 배열 JSON을 트랜잭션으로 적용한다. BatchResult JSON 반환.
    pub fn apply_actions(&mut self, json: &str) -> Result<String, JsError> {
        let actions: Vec<Action> =
            serde_json::from_str(json).map_err(|e| JsError::new(&format!("Action JSON 파싱: {e}")))?;
        let res = dispatch::apply_batch(&mut self.hist, &actions, false);
        if res.ok {
            self.dirty = true;
        }
        serde_json::to_string(&res).map_err(|e| JsError::new(&e.to_string()))
    }

    /// 검증만(무변경). BatchResult JSON 반환.
    pub fn dry_run(&mut self, json: &str) -> Result<String, JsError> {
        let actions: Vec<Action> =
            serde_json::from_str(json).map_err(|e| JsError::new(&format!("Action JSON 파싱: {e}")))?;
        let res = dispatch::apply_batch(&mut self.hist, &actions, true);
        serde_json::to_string(&res).map_err(|e| JsError::new(&e.to_string()))
    }

    /// 마지막 논리 단위(단발 op 또는 batch 전체)를 되돌린다. 빈 스택이면 Ok(false).
    pub fn undo(&mut self) -> Result<bool, JsError> {
        let r = self.hist.undo().map_err(|e| JsError::new(&e.to_string()))?;
        if r {
            self.dirty = true;
        }
        Ok(r)
    }

    pub fn redo(&mut self) -> Result<bool, JsError> {
        let r = self.hist.redo().map_err(|e| JsError::new(&e.to_string()))?;
        if r {
            self.dirty = true;
        }
        Ok(r)
    }

    pub fn can_undo(&self) -> bool {
        self.hist.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.hist.can_redo()
    }

    /// 문서 메타 JSON.
    pub fn doc_info(&self) -> String {
        dto::doc_info_json(&self.hist.doc).to_string()
    }

    /// 레이어 목록 JSON(bottom-to-top).
    pub fn layers(&self) -> String {
        dto::layer_list_json(&self.hist.doc).to_string()
    }

    /// 캔버스 좌표 (x,y)에서 hit되는 최상위(top) 가시 레이어 node id를 반환한다.
    /// 트랜스폼(offset/scale/rotation)을 역적용해 표면 픽셀 alpha>0이면 hit.
    /// 없으면 -1(선택 해제). id는 JS 친화 i32(node id는 작은 값 가정).
    pub fn hit_test(&self, x: i32, y: i32) -> i32 {
        use dcli_model::NodeKind;
        // top-to-bottom: order는 bottom-to-top이라 역순.
        for &id in self.hist.doc.order().iter().rev() {
            let Some(node) = self.hist.doc.get(id) else { continue };
            if !node.visible || node.opacity <= 0.0 {
                continue;
            }
            let NodeKind::Paint { surface } = node.kind else { continue };
            let Some(surf) = self.hist.doc.pixels().get(surface) else { continue };
            let (swi, shi) = (surf.width() as i32, surf.height() as i32);
            let (psx, psy) = if node.is_identity_transform() {
                ((x - node.offset.0), (y - node.offset.1))
            } else {
                // 역변환(raster composite_layer_transformed와 동일 수학) 후 floor.
                let (scx, scy) = node.scale;
                if scx.abs() < 1e-4 || scy.abs() < 1e-4 {
                    continue;
                }
                let (sin, cos) = node.rotation.to_radians().sin_cos();
                let (cx, cy) = (swi as f32 * 0.5, shi as f32 * 0.5);
                let qx = x as f32 + 0.5 - node.offset.0 as f32 - cx;
                let qy = y as f32 + 0.5 - node.offset.1 as f32 - cy;
                let rx = cos * qx + sin * qy;
                let ry = -sin * qx + cos * qy;
                ((rx / scx + cx).floor() as i32, (ry / scy + cy).floor() as i32)
            };
            if psx < 0 || psy < 0 || psx >= swi || psy >= shi {
                continue;
            }
            let px = surf.pixels()[(psy * swi + psx) as usize];
            if px.a > 0.0 {
                return id.0 as i32;
            }
        }
        -1
    }

    /// 레이어의 불투명 픽셀 타이트 바운드 — **표면(src) 좌표, 트랜스폼 미적용** [x,y,w,h].
    /// 웹이 dto의 offset/scale/rotation으로 4코너를 변환해 회전 셀렉션 박스를 그린다.
    /// 빈 레이어면 null. id는 JS 친화 u32.
    pub fn layer_bounds(&self, id: u32) -> String {
        use dcli_model::{NodeId, NodeKind};
        let Some(node) = self.hist.doc.get(NodeId(id as u64)) else { return "null".into() };
        let NodeKind::Paint { surface } = node.kind else { return "null".into() };
        let Some(surf) = self.hist.doc.pixels().get(surface) else { return "null".into() };
        let (w, h) = (surf.width() as i32, surf.height() as i32);
        let px = surf.pixels();
        let (mut minx, mut miny, mut maxx, mut maxy) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for y in 0..h {
            for x in 0..w {
                if px[(y * w + x) as usize].a > 0.0 {
                    minx = minx.min(x);
                    miny = miny.min(y);
                    maxx = maxx.max(x);
                    maxy = maxy.max(y);
                }
            }
        }
        if minx > maxx {
            return "null".into(); // 완전 투명.
        }
        format!("[{},{},{},{}]", minx, miny, maxx - minx + 1, maxy - miny + 1)
    }

    pub fn width(&self) -> u32 {
        self.hist.doc.width
    }

    pub fn height(&self) -> u32 {
        self.hist.doc.height
    }

    /// 합성 결과를 straight-alpha sRGB8 RGBA로 반환(canvas ImageData용, 복사본).
    pub fn composite_rgba(&mut self) -> js_sys::Uint8ClampedArray {
        self.ensure_composited();
        js_sys::Uint8ClampedArray::from(self.rgba.as_slice())
    }

    /// 합성 결과를 PNG 바이트로 반환(결정적 export).
    pub fn export_png(&mut self) -> Result<Vec<u8>, JsError> {
        self.ensure_composited();
        let (w, h) = (self.width(), self.height());
        let mut png = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()
                .and_then(|mut w| w.write_image_data(&self.rgba))
                .map_err(|e| JsError::new(&e.to_string()))?;
        }
        Ok(png)
    }

    /// 문서를 .dxpkg 단일 파일 바이트로 직렬화(저장). 코덱은 dcli_cli::dxpkg 공유.
    pub fn to_dxpkg(&self) -> Vec<u8> {
        dxpkg::encode(&self.hist.doc)
    }

    /// .dxpkg 바이트에서 새 Editor를 만든다(열기). 코덱은 dcli_cli::dxpkg 공유.
    pub fn from_dxpkg(bytes: &[u8]) -> Result<Editor, JsError> {
        let doc = dxpkg::decode(bytes).map_err(|e| JsError::new(&e))?;
        Ok(Editor { hist: History::new(doc), rgba: Vec::new(), dirty: true })
    }

    fn ensure_composited(&mut self) {
        if self.dirty || self.rgba.is_empty() {
            self.rgba = dcli_raster::composite(&self.hist.doc).to_srgb8_rgba();
            self.dirty = false;
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn __start() {
    console_error_panic_hook::set_once();
}

// .dxpkg 코덱은 dcli_cli::dxpkg로 이동(스냅샷 포맷 단일 진실원 — 데몬·CLI·wasm 공유).

// ---- host(네이티브 rlib) 회귀 테스트 ----
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_strict_rejects_unknown() {
        // 순수 파서를 직접 검증(JsError는 host에서 패닉하므로 우회).
        assert!(parse_depth("rgb").is_err(), "알 수 없는 depth는 Err");
        assert!(parse_depth("u8").is_ok());
        assert!(parse_depth("u16").is_ok());
        assert!(parse_depth("f32").is_ok());
    }

    #[test]
    fn apply_actions_json_roundtrip() {
        let mut ed = Editor::new(16, 16, "u8").unwrap();
        let json = r#"[{"op":"add_paint_layer","name":"bg","source":{"from":"fill","rgba":[255,0,0,255]}}]"#;
        let res = ed.apply_actions(json).unwrap();
        let v: serde_json::Value = serde_json::from_str(&res).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["applied"], 1);
        // 레이어 목록에 반영.
        let layers: serde_json::Value = serde_json::from_str(&ed.layers()).unwrap();
        assert_eq!(layers["layers"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn premul_to_straight_pin() {
        // ★검증 #1 핀★ 반투명 빨강을 그리면 straight 색이어야(R≈255, A≈128).
        // premul 누수면 R이 ~128로 어두워진다.
        let mut ed = Editor::new(4, 4, "u8").unwrap();
        let json = r#"[{"op":"add_paint_layer","name":"x","source":{"from":"fill","rgba":[255,0,0,128]}}]"#;
        ed.apply_actions(json).unwrap();
        ed.ensure_composited();
        let px = &ed.rgba[0..4];
        assert!(px[0] > 245, "straight 빨강 R≈255 (premul 누수 아님): {}", px[0]);
        assert!((px[3] as i32 - 128).abs() <= 2, "alpha≈128: {}", px[3]);
    }

    #[test]
    fn undo_redo_and_error_distinct_from_empty() {
        let mut ed = Editor::new(8, 8, "u8").unwrap();
        // 빈 스택 undo = Ok(false), 에러 아님.
        assert!(!ed.undo().unwrap());
        let json = r#"[{"op":"add_paint_layer","name":"a","source":{"from":"transparent"}}]"#;
        ed.apply_actions(json).unwrap();
        assert!(ed.can_undo());
        assert!(ed.undo().unwrap());
        assert!(!ed.undo().unwrap(), "다시 빈 스택");
        assert!(ed.redo().unwrap());
    }

    #[test]
    fn dxpkg_roundtrip_bit_identical() {
        let mut ed = Editor::new(12, 12, "u8").unwrap();
        ed.apply_actions(
            r#"[{"op":"add_paint_layer","name":"bg","source":{"from":"fill","rgba":[30,60,90,255]}},
                {"op":"add_paint_layer","name":"art","source":{"from":"shapes","items":[
                  {"shape":"ellipse","cx":6,"cy":6,"rx":4,"ry":4,"rgba":[255,200,0,255]}]}}]"#,
        )
        .unwrap();
        let before = dcli_raster::composite(&ed.hist.doc).to_srgb8_rgba();

        let pkg = ed.to_dxpkg();
        let ed2 = Editor::from_dxpkg(&pkg).unwrap();
        let after = dcli_raster::composite(&ed2.hist.doc).to_srgb8_rgba();
        assert_eq!(before, after, ".dxpkg 라운드트립 후 합성 비트동일");
    }

    #[test]
    fn blend_str_matches_action_casing() {
        // 패널 JSON의 blend("multiply")를 그대로 set_blend Action에 넣어 역직렬화 성공.
        let mut ed = Editor::new(8, 8, "u8").unwrap();
        ed.apply_actions(r#"[{"op":"add_paint_layer","name":"a","source":{"from":"transparent"}}]"#)
            .unwrap();
        ed.apply_actions(r#"[{"op":"set_blend","id":{"node":0},"mode":"multiply"}]"#)
            .unwrap();
        let layers: serde_json::Value = serde_json::from_str(&ed.layers()).unwrap();
        assert_eq!(layers["layers"][0]["blend"], "multiply", "snake_case 케이싱 일치");
    }
}
