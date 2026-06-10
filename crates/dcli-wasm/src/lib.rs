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

    /// 캔버스 좌표 (x,y)에서 hit되는 최상위(top) 가시 노드 id를 반환한다.
    /// 그룹은 자식 어느 하나라도 hit이면 **그룹 id**를 반환(Figma처럼 그룹 단위 선택).
    /// 트랜스폼(offset/scale/rotation)을 역적용해 표면 픽셀 alpha>0이면 hit.
    /// 없으면 -1(선택 해제). id는 JS 친화 i32(node id는 작은 값 가정).
    pub fn hit_test(&self, x: i32, y: i32) -> i32 {
        let p = (x as f32 + 0.5, y as f32 + 0.5);
        for &id in self.hist.doc.order().iter().rev() {
            let Some(node) = self.hist.doc.get(id) else { continue };
            if hit_node(&self.hist.doc, node, p, 0) {
                return id.0 as i32;
            }
        }
        -1
    }

    /// 레이어의 불투명 픽셀 타이트 바운드 — **표면(src) 좌표, 자기 트랜스폼 미적용** [x,y,w,h].
    /// 그룹은 자식들의 (자식 트랜스폼 적용된) 바운드 합집합 — 그룹 자신의 트랜스폼은 미적용.
    /// 웹이 dto의 offset/scale/rotation으로 4코너를 변환해 회전 셀렉션 박스를 그린다.
    /// 빈 레이어면 null. id는 JS 친화 u32.
    pub fn layer_bounds(&self, id: u32) -> String {
        use dcli_model::NodeId;
        let Some(node) = self.hist.doc.get(NodeId(id as u64)) else { return "null".into() };
        match node_src_bounds(&self.hist.doc, node, 0) {
            Some((x0, y0, x1, y1)) => format!("[{},{},{},{}]", x0, y0, x1 - x0, y1 - y0),
            None => "null".into(),
        }
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

    /// 특정 레이어를 **화면에서만 제외**하고 합성(읽기 전용 — 문서·undo·동기화 무오염).
    /// 텍스트 인라인 편집 중 원본이 밑에 비치는 것을 막는 용도. 캐시 안 씀.
    pub fn composite_rgba_excluding(&self, id: u32) -> js_sys::Uint8ClampedArray {
        use dcli_model::NodeId;
        let mut doc = self.hist.doc.clone();
        if let Some(n) = doc.get_mut(NodeId(id as u64)) {
            n.visible = false;
        }
        let rgba = dcli_raster::composite(&doc).to_srgb8_rgba();
        js_sys::Uint8ClampedArray::from(rgba.as_slice())
    }

    /// 임의 영역 합성(무한 작업영역 뷰포트/Frame 미리보기) — straight sRGB8 RGBA.
    pub fn composite_region_rgba(&self, x: i32, y: i32, w: u32, h: u32) -> js_sys::Uint8ClampedArray {
        let rgba = dcli_raster::composite_region(&self.hist.doc, x, y, w, h).to_srgb8_rgba();
        js_sys::Uint8ClampedArray::from(rgba.as_slice())
    }

    /// 특정 레이어를 화면에서만 제외하고 임의 영역 합성.
    pub fn composite_region_rgba_excluding(
        &self,
        id: u32,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> js_sys::Uint8ClampedArray {
        use dcli_model::NodeId;
        let mut doc = self.hist.doc.clone();
        if let Some(n) = doc.get_mut(NodeId(id as u64)) {
            n.visible = false;
        }
        let rgba = dcli_raster::composite_region(&doc, x, y, w, h).to_srgb8_rgba();
        js_sys::Uint8ClampedArray::from(rgba.as_slice())
    }

    /// 임의 영역을 PNG로(Frame 단위 export).
    pub fn export_region_png(&self, x: i32, y: i32, w: u32, h: u32) -> Result<Vec<u8>, JsError> {
        let rgba = dcli_raster::composite_region(&self.hist.doc, x, y, w, h).to_srgb8_rgba();
        let mut png = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()
                .and_then(|mut wr| wr.write_image_data(&rgba))
                .map_err(|e| JsError::new(&e.to_string()))?;
        }
        Ok(png)
    }

    /// Frame 목록 JSON.
    pub fn frames(&self) -> String {
        dto::frames_json(&self.hist.doc).to_string()
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

/// 노드(트랜스폼 포함)가 월드 점 p를 덮는지 — 그룹은 자식 재귀(역변환 후).
fn hit_node(doc: &dcli_model::Document, node: &dcli_model::Node, p: (f32, f32), depth: u32) -> bool {
    use dcli_model::NodeKind;
    if !node.visible || node.opacity <= 0.0 || depth > 32 {
        return false;
    }
    // 노드 자신의 트랜스폼 역적용(엔진과 동일 수학 — 중심 = 문서 중심).
    let (dw, dh) = (doc.width as f32, doc.height as f32);
    let local = if node.is_identity_transform() {
        (p.0 - node.offset.0 as f32, p.1 - node.offset.1 as f32)
    } else {
        let (scx, scy) = node.scale;
        if scx.abs() < 1e-4 || scy.abs() < 1e-4 {
            return false;
        }
        let (sin, cos) = node.rotation.to_radians().sin_cos();
        let (cx, cy) = (dw * 0.5, dh * 0.5);
        let qx = p.0 - node.offset.0 as f32 - cx;
        let qy = p.1 - node.offset.1 as f32 - cy;
        (
            (cos * qx + sin * qy) / scx + cx,
            (-sin * qx + cos * qy) / scy + cy,
        )
    };
    match &node.kind {
        NodeKind::Paint { surface } => {
            let Some(surf) = doc.pixels().get(*surface) else { return false };
            let (sx, sy) = (local.0.floor() as i32, local.1.floor() as i32);
            if sx < 0 || sy < 0 || sx >= surf.width() as i32 || sy >= surf.height() as i32 {
                return false;
            }
            surf.pixels()[(sy * surf.width() as i32 + sx) as usize].a > 0.0
        }
        NodeKind::Group { children } => children
            .iter()
            .rev()
            .filter_map(|cid| doc.get(*cid))
            .any(|child| hit_node(doc, child, local, depth + 1)),
    }
}

/// 노드의 src-공간 타이트 바운드(자기 트랜스폼 미적용) (x0,y0,x1,y1) — 그룹은 자식
/// 바운드(자식 트랜스폼 적용)의 합집합.
fn node_src_bounds(
    doc: &dcli_model::Document,
    node: &dcli_model::Node,
    depth: u32,
) -> Option<(i32, i32, i32, i32)> {
    use dcli_model::NodeKind;
    if depth > 32 {
        return None;
    }
    match &node.kind {
        NodeKind::Paint { surface } => {
            let surf = doc.pixels().get(*surface)?;
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
            (minx <= maxx).then_some((minx, miny, maxx + 1, maxy + 1))
        }
        NodeKind::Group { children } => {
            let (dw, dh) = (doc.width as f32, doc.height as f32);
            let (cx, cy) = (dw * 0.5, dh * 0.5);
            let mut acc: Option<(f32, f32, f32, f32)> = None;
            for child in children.iter().filter_map(|cid| doc.get(*cid)) {
                let Some((x0, y0, x1, y1)) = node_src_bounds(doc, child, depth + 1) else {
                    continue;
                };
                // 자식 자신의 트랜스폼으로 4코너를 월드로(그룹-로컬 = 월드 기준).
                let (sin, cos) = child.rotation.to_radians().sin_cos();
                let (scx, scy) = child.scale;
                let (ox, oy) = (child.offset.0 as f32, child.offset.1 as f32);
                let fwd = |px: f32, py: f32| -> (f32, f32) {
                    let vx = (px - cx) * scx;
                    let vy = (py - cy) * scy;
                    (cos * vx - sin * vy + cx + ox, sin * vx + cos * vy + cy + oy)
                };
                for (px, py) in [
                    fwd(x0 as f32, y0 as f32),
                    fwd(x1 as f32, y0 as f32),
                    fwd(x0 as f32, y1 as f32),
                    fwd(x1 as f32, y1 as f32),
                ] {
                    acc = Some(match acc {
                        None => (px, py, px, py),
                        Some((a, b, c, d)) => (a.min(px), b.min(py), c.max(px), d.max(py)),
                    });
                }
            }
            acc.map(|(a, b, c, d)| (a.floor() as i32, b.floor() as i32, c.ceil() as i32, d.ceil() as i32))
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
