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
    /// 뷰 벡터 재래스터 캐시 — (surface id, meta 해시, scale bits) → (표면, 스케일 월드 원점).
    /// 같은 줌에서 팬·재합성 시 재사용하고, 편집(표면 교체)·meta 변경 시 키가 자연 무효화된다.
    view_cache: std::cell::RefCell<
        std::collections::HashMap<(u64, u64, u32), (std::rc::Rc<dcli_tile::Surface>, (i32, i32))>,
    >,
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
        Ok(Editor {
            hist: History::new(doc),
            rgba: Vec::new(),
            dirty: true,
            view_cache: Default::default(),
        })
    }

    /// Action 배열 JSON을 트랜잭션으로 적용한다. BatchResult JSON 반환.
    pub fn apply_actions(&mut self, json: &str) -> Result<String, JsError> {
        let actions: Vec<Action> = serde_json::from_str(json)
            .map_err(|e| JsError::new(&format!("Action JSON 파싱: {e}")))?;
        let res = dispatch::apply_batch(&mut self.hist, &actions, false);
        if res.ok {
            self.dirty = true;
        }
        serde_json::to_string(&res).map_err(|e| JsError::new(&e.to_string()))
    }

    /// 검증만(무변경). BatchResult JSON 반환.
    pub fn dry_run(&mut self, json: &str) -> Result<String, JsError> {
        let actions: Vec<Action> = serde_json::from_str(json)
            .map_err(|e| JsError::new(&format!("Action JSON 파싱: {e}")))?;
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
            let Some(node) = self.hist.doc.get(id) else {
                continue;
            };
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
        let Some(node) = self.hist.doc.get(NodeId(id as u64)) else {
            return "null".into();
        };
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
    pub fn composite_region_rgba(
        &self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> js_sys::Uint8ClampedArray {
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

    /// 화면(뷰) 합성 — 보이는 영역만 출력 해상도로 직접 그린다.
    /// (vx, vy) = 출력 (0,0)의 월드 좌표, s = 줌×렌더스케일(출력 1px = 1/s 문서 px).
    /// 벡터 meta(도형/브러시/텍스트) 레이어는 뷰 배율로 재래스터 — 확대 계단현상 없음.
    pub fn composite_view_rgba(
        &self,
        vx: f32,
        vy: f32,
        s: f32,
        w: u32,
        h: u32,
    ) -> js_sys::Uint8ClampedArray {
        let doc = &self.hist.doc;
        let rgba = dcli_raster::composite_view_with(doc, vx, vy, s, w, h, &|n, sc| {
            self.vector_render(doc, n, sc)
        })
        .to_srgb8_rgba_fast();
        js_sys::Uint8ClampedArray::from(rgba.as_slice())
    }

    /// 뷰 합성 + 노드 1개 화면 제외(텍스트 인라인 편집용).
    pub fn composite_view_rgba_excluding(
        &self,
        id: u32,
        vx: f32,
        vy: f32,
        s: f32,
        w: u32,
        h: u32,
    ) -> js_sys::Uint8ClampedArray {
        use dcli_model::NodeId;
        let mut doc = self.hist.doc.clone();
        if let Some(n) = doc.get_mut(NodeId(id as u64)) {
            n.visible = false;
        }
        let rgba = {
            let doc_ref = &doc;
            dcli_raster::composite_view_with(doc_ref, vx, vy, s, w, h, &|n, sc| {
                self.vector_render(doc_ref, n, sc)
            })
            .to_srgb8_rgba_fast()
        };
        js_sys::Uint8ClampedArray::from(rgba.as_slice())
    }

    /// 뷰 합성 결과를 PNG로 — 루프 엔지니어링 시각 검수 산출물용(render_scene.mjs).
    pub fn composite_view_png(
        &self,
        vx: f32,
        vy: f32,
        s: f32,
        w: u32,
        h: u32,
    ) -> Result<Vec<u8>, JsError> {
        let doc = &self.hist.doc;
        let rgba = dcli_raster::composite_view_with(doc, vx, vy, s, w, h, &|n, sc| {
            self.vector_render(doc, n, sc)
        })
        .to_srgb8_rgba();
        let mut png = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut wtr = enc
                .write_header()
                .map_err(|e| JsError::new(&format!("PNG 헤더: {e}")))?;
            wtr.write_image_data(&rgba)
                .map_err(|e| JsError::new(&format!("PNG 인코딩: {e}")))?;
        }
        Ok(png)
    }

    /// 문서를 PSD 바이트로 export(레이어·블렌드·불투명도 보존 — dcli-psd 인코더).
    pub fn to_psd(&self) -> Vec<u8> {
        dcli_psd::export_psd(&self.hist.doc)
    }

    /// 텍스트 레이아웃 측정 [w, h] — 엔진과 동일 metric(텍스트 배경 박스 구성용).
    pub fn measure_text(&self, text: &str, size: f32, font: Option<String>) -> Vec<f32> {
        let (w, h) = dcli_raster::text::measure_text_font(text, size, font.as_deref());
        vec![w, h]
    }

    /// 글꼴 등록(TTF/OTF/TTC 바이트). 등록 후 벡터 캐시를 비워 즉시 재래스터되게 한다.
    pub fn register_font(&mut self, name: &str, bytes: &[u8], face_index: u32) -> Result<(), JsError> {
        dcli_raster::text::register_font(name, bytes.to_vec(), face_index)
            .map_err(|e| JsError::new(&e))?;
        self.view_cache.borrow_mut().clear();
        self.dirty = true;
        Ok(())
    }

    /// 레이어 1개만 PNG로 export(피그마의 per-layer Export). 트랜스폼 적용된 표시 AABB
    /// 영역을, 해당 노드(그룹이면 서브트리 + 조상)만 보이게 한 문서로 합성한다.
    pub fn export_layer_png(&self, id: u32) -> Result<Vec<u8>, JsError> {
        use dcli_model::{NodeId, NodeKind};
        use std::collections::HashSet;
        let target = NodeId(id as u64);
        let mut doc = self.hist.doc.clone();
        // 보존 집합 = target 서브트리 + 조상 경로(그룹 안이어도 보이게).
        fn collect(
            doc: &dcli_model::Document,
            id: NodeId,
            target: NodeId,
            path: &mut Vec<u64>,
            keep: &mut HashSet<u64>,
            in_subtree: bool,
        ) -> bool {
            path.push(id.0);
            let mine = id == target || in_subtree;
            if mine {
                keep.insert(id.0);
            }
            let mut found = id == target;
            if let Some(node) = doc.get(id) {
                if let NodeKind::Group { children } = &node.kind {
                    for c in children.clone() {
                        found |= collect(doc, c, target, path, keep, mine);
                    }
                }
            }
            if found {
                keep.extend(path.iter().copied());
            }
            path.pop();
            found
        }
        let mut keep = HashSet::new();
        let roots: Vec<NodeId> = doc.order().to_vec();
        let mut found = false;
        for r in &roots {
            found |= collect(&doc, *r, target, &mut Vec::new(), &mut keep, false);
        }
        if !found {
            return Err(JsError::new(&format!("레이어 없음: n{id}")));
        }
        fn hide_others(doc: &mut dcli_model::Document, id: NodeId, keep: &HashSet<u64>) {
            let children = match doc.get(id).map(|n| n.kind.clone()) {
                Some(NodeKind::Group { children }) => children,
                _ => Vec::new(),
            };
            if !keep.contains(&id.0) {
                if let Some(n) = doc.get_mut(id) {
                    n.visible = false;
                }
            }
            for c in children {
                hide_others(doc, c, keep);
            }
        }
        for r in roots {
            hide_others(&mut doc, r, &keep);
        }
        // 표시 AABB(트랜스폼 적용) 영역 계산.
        let node = doc
            .get(target)
            .ok_or_else(|| JsError::new(&format!("레이어 없음: n{id}")))?;
        let (bx0, by0, bx1, by1) = node_src_bounds(&doc, node, 0)
            .ok_or_else(|| JsError::new("빈 레이어 — export할 픽셀 없음"))?;
        let (sin, cos) = node.rotation.to_radians().sin_cos();
        let (sx, sy) = node.scale;
        let (ox, oy) = (node.offset.0 as f32, node.offset.1 as f32);
        let (cx, cy) = transform_center(&doc, node);
        let fwd = |px: f32, py: f32| -> (f32, f32) {
            let vx = (px - cx) * sx;
            let vy = (py - cy) * sy;
            (cos * vx - sin * vy + cx + ox, sin * vx + cos * vy + cy + oy)
        };
        let cs = [
            fwd(bx0 as f32, by0 as f32),
            fwd(bx1 as f32, by0 as f32),
            fwd(bx0 as f32, by1 as f32),
            fwd(bx1 as f32, by1 as f32),
        ];
        let minx = cs.iter().map(|p| p.0).fold(f32::INFINITY, f32::min).floor() as i32;
        let miny = cs.iter().map(|p| p.1).fold(f32::INFINITY, f32::min).floor() as i32;
        let maxx = cs.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max).ceil() as i32;
        let maxy = cs.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max).ceil() as i32;
        let (w, h) = (((maxx - minx).max(1)) as u32, ((maxy - miny).max(1)) as u32);
        self.encode_png(dcli_raster::composite_region(&doc, minx, miny, w, h).to_srgb8_rgba(), w, h)
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
        Ok(Editor {
            hist: History::new(doc),
            rgba: Vec::new(),
            dirty: true,
            view_cache: Default::default(),
        })
    }

    fn ensure_composited(&mut self) {
        if self.dirty || self.rgba.is_empty() {
            self.rgba = dcli_raster::composite(&self.hist.doc).to_srgb8_rgba();
            self.dirty = false;
        }
    }
}

impl Editor {
    /// RGBA 바이트 → PNG 인코딩(공용).
    fn encode_png(&self, rgba: Vec<u8>, w: u32, h: u32) -> Result<Vec<u8>, JsError> {
        let mut png = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut wtr = enc
                .write_header()
                .map_err(|e| JsError::new(&format!("PNG 헤더: {e}")))?;
            wtr.write_image_data(&rgba)
                .map_err(|e| JsError::new(&format!("PNG 인코딩: {e}")))?;
        }
        Ok(png)
    }

    /// 벡터 레이어 재래스터(+캐시) — **전 배율**. 확대는 계단 제거, 축소는 타깃 해상도
    /// AA(4-tap 샘플의 줌아웃 계단 제거). 키 = (표면 id, meta+offset 해시, scale bits):
    /// 편집(표면 교체)·이동·meta·줌 변경 시 자연 무효화, 같은 줌의 팬·재합성은 블릿만.
    fn vector_render(
        &self,
        doc: &dcli_model::Document,
        node: &dcli_model::Node,
        s: f32,
    ) -> Option<(std::rc::Rc<dcli_tile::Surface>, (i32, i32))> {
        use std::hash::{Hash, Hasher};
        let dcli_model::NodeKind::Paint { surface } = node.kind else {
            return None;
        };
        let meta = node.meta.as_deref()?;
        let mut hsh = std::collections::hash_map::DefaultHasher::new();
        meta.hash(&mut hsh);
        node.offset.hash(&mut hsh); // 이동 시 월드 좌표가 바뀌므로 키에 포함.
        let key = (surface.0, hsh.finish(), s.to_bits());
        if let Some(hit) = self.view_cache.borrow().get(&key) {
            return Some(hit.clone());
        }
        let items = vector_items_of(doc, node)?;
        let (sfc, origin) = dcli_raster::render_view_items(&items, s, 16_000_000)?;
        let entry = (std::rc::Rc::new(sfc), origin);
        let mut cache = self.view_cache.borrow_mut();
        if cache.len() > 128 {
            cache.clear();
        }
        cache.insert(key, entry.clone());
        Some(entry)
    }
}

/// node.meta가 기술하는 벡터 아이템들을 **월드 좌표**로 복원한다(뷰 재래스터용).
///
/// 아이템 재구성은 dispatch::items_from_meta(단일 출처 — CLI layer style과 공유)가 한다.
/// 좌표 규약(editor-coordinate-contracts): 월드 = 아이템 + (offset − origin),
/// 레거시(문서 크기 표면)는 origin = (0,0). 해석 불가/비벡터 meta는 None → 래스터 폴백.
fn vector_items_of(
    doc: &dcli_model::Document,
    node: &dcli_model::Node,
) -> Option<Vec<dcli_raster::ViewItem>> {
    use dcli_model::NodeKind;
    let NodeKind::Paint { surface } = node.kind else {
        return None;
    };
    let meta = node.meta.as_deref()?;
    let m: serde_json::Value = serde_json::from_str(meta).ok()?;
    let shapes = dcli_cli::dispatch::items_from_meta(&m)?;
    let surf = doc.pixels().get(surface)?;
    let doc_sized = surf.width() == doc.width && surf.height() == doc.height;
    let origin = if doc_sized {
        (0, 0)
    } else {
        dcli_cli::dispatch::shapes_origin(&shapes)?
    };
    let dx = (node.offset.0 - origin.0) as f32;
    let dy = (node.offset.1 - origin.1) as f32;
    Some(
        shapes
            .into_iter()
            .map(|sh| shape_to_view(sh, dx, dy))
            .collect(),
    )
}

fn grad_to_view(g: dcli_cli::dispatch::GradFill) -> dcli_raster::ViewGrad {
    dcli_raster::ViewGrad {
        x0: g.x0,
        y0: g.y0,
        x1: g.x1,
        y1: g.y1,
        radial: g.radial,
        stops: g.stops.into_iter().map(|st| (st.at, st.rgba)).collect(),
    }
}

fn shape_to_view(sh: dcli_cli::dispatch::Shape, dx: f32, dy: f32) -> dcli_raster::ViewItem {
    use dcli_cli::dispatch::Shape as S;
    use dcli_raster::ViewItem as V;
    match sh {
        S::Rect { x, y, w, h, rgba, gradient } => V::Rect {
            x: x + dx,
            y: y + dy,
            w,
            h,
            rgba,
            gradient: gradient.map(grad_to_view),
        },
        S::RoundedRect {
            x,
            y,
            w,
            h,
            radius,
            rgba,
            gradient,
        } => V::RoundedRect {
            x: x + dx,
            y: y + dy,
            w,
            h,
            radius,
            rgba,
            gradient: gradient.map(grad_to_view),
        },
        S::StrokeRect {
            x,
            y,
            w,
            h,
            width,
            rgba,
        } => V::StrokeRect {
            x: x + dx,
            y: y + dy,
            w,
            h,
            width,
            rgba,
        },
        S::StrokeRoundedRect {
            x,
            y,
            w,
            h,
            radius,
            width,
            rgba,
        } => V::StrokeRoundedRect {
            x: x + dx,
            y: y + dy,
            w,
            h,
            radius,
            width,
            rgba,
        },
        S::Ellipse {
            cx,
            cy,
            rx,
            ry,
            rgba,
            gradient,
        } => V::Ellipse {
            cx: cx + dx,
            cy: cy + dy,
            rx,
            ry,
            rgba,
            gradient: gradient.map(grad_to_view),
        },
        S::StrokeEllipse {
            cx,
            cy,
            rx,
            ry,
            width,
            rgba,
        } => V::StrokeEllipse {
            cx: cx + dx,
            cy: cy + dy,
            rx,
            ry,
            width,
            rgba,
        },
        S::Line {
            x0,
            y0,
            x1,
            y1,
            width,
            rgba,
        } => V::Line {
            x0: x0 + dx,
            y0: y0 + dy,
            x1: x1 + dx,
            y1: y1 + dy,
            width,
            rgba,
        },
        S::Path {
            points,
            width,
            rgba,
        } => V::Path {
            points: points
                .chunks(2)
                .flat_map(|p| [p[0] + dx, p[1] + dy])
                .collect(),
            width,
            rgba,
        },
        S::Text {
            x,
            y,
            text,
            size,
            rgba,
            font,
        } => V::Text {
            x: x + dx,
            y: y + dy,
            text,
            size,
            rgba,
            font,
        },
        S::Shadow {
            x,
            y,
            w,
            h,
            radius,
            feather,
            rgba,
        } => V::Shadow {
            x: x + dx,
            y: y + dy,
            w,
            h,
            radius,
            feather,
            rgba,
        },
    }
}

fn transform_center(doc: &dcli_model::Document, node: &dcli_model::Node) -> (f32, f32) {
    use dcli_model::NodeKind;
    match &node.kind {
        NodeKind::Paint { surface } => doc
            .pixels()
            .get(*surface)
            .map(|surf| (surf.width() as f32 * 0.5, surf.height() as f32 * 0.5))
            .unwrap_or((doc.width as f32 * 0.5, doc.height as f32 * 0.5)),
        NodeKind::Group { .. } => (doc.width as f32 * 0.5, doc.height as f32 * 0.5),
    }
}

/// 노드(트랜스폼 포함)가 월드 점 p를 덮는지 — 그룹은 자식 재귀(역변환 후).
fn hit_node(
    doc: &dcli_model::Document,
    node: &dcli_model::Node,
    p: (f32, f32),
    depth: u32,
) -> bool {
    use dcli_model::NodeKind;
    if !node.visible || node.opacity <= 0.0 || depth > 32 {
        return false;
    }
    // 노드 자신의 트랜스폼 역적용(엔진과 동일 수학 — 중심 = 표면 중심).
    let local = if node.is_identity_transform() {
        (p.0 - node.offset.0 as f32, p.1 - node.offset.1 as f32)
    } else {
        let (scx, scy) = node.scale;
        if scx.abs() < 1e-4 || scy.abs() < 1e-4 {
            return false;
        }
        let (sin, cos) = node.rotation.to_radians().sin_cos();
        let (cx, cy) = transform_center(doc, node);
        let qx = p.0 - node.offset.0 as f32 - cx;
        let qy = p.1 - node.offset.1 as f32 - cy;
        (
            (cos * qx + sin * qy) / scx + cx,
            (-sin * qx + cos * qy) / scy + cy,
        )
    };
    match &node.kind {
        NodeKind::Paint { surface } => {
            let Some(surf) = doc.pixels().get(*surface) else {
                return false;
            };
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
            let mut acc: Option<(f32, f32, f32, f32)> = None;
            for child in children.iter().filter_map(|cid| doc.get(*cid)) {
                let Some((x0, y0, x1, y1)) = node_src_bounds(doc, child, depth + 1) else {
                    continue;
                };
                // 자식 자신의 트랜스폼으로 4코너를 월드로(그룹-로컬 = 월드 기준).
                let (sin, cos) = child.rotation.to_radians().sin_cos();
                let (scx, scy) = child.scale;
                let (ox, oy) = (child.offset.0 as f32, child.offset.1 as f32);
                let (cx, cy) = transform_center(doc, child);
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
            acc.map(|(a, b, c, d)| {
                (
                    a.floor() as i32,
                    b.floor() as i32,
                    c.ceil() as i32,
                    d.ceil() as i32,
                )
            })
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
        assert!(
            px[0] > 245,
            "straight 빨강 R≈255 (premul 누수 아님): {}",
            px[0]
        );
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
        ed.apply_actions(
            r#"[{"op":"add_paint_layer","name":"a","source":{"from":"transparent"}}]"#,
        )
        .unwrap();
        ed.apply_actions(r#"[{"op":"set_blend","id":{"node":0},"mode":"multiply"}]"#)
            .unwrap();
        let layers: serde_json::Value = serde_json::from_str(&ed.layers()).unwrap();
        assert_eq!(
            layers["layers"][0]["blend"], "multiply",
            "snake_case 케이싱 일치"
        );
    }
}
