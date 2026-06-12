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
    /// 보존 프레임(마지막 뷰 합성 결과) — 팬은 스크롤, 편집은 손상 rect만 재합성.
    frame: std::cell::RefCell<Option<Retained>>,
    /// 마지막 프레임 이후 누적 손상(월드 좌표). 기본 full(첫 프레임 전체 합성).
    damage: std::cell::RefCell<DamageLog>,
    /// meta 벡터 아이템의 월드 경계 캐시 — (meta+offset 해시) → bbox. 컬링·그룹 tmp·
    /// 손상영역이 표면보다 넓은 meta(그림자 등 set_props-only 흐름)를 안 놓치게 한다.
    bounds_cache: std::cell::RefCell<std::collections::HashMap<u64, Option<(f32, f32, f32, f32)>>>,
}

/// 화면 보존 프레임 — 마지막으로 합성한 sRGB8 버퍼와 그 뷰 파라미터.
/// 원점은 디바이스 정수 격자로 양자화되어 저장된다(스크롤·sub-rect 합성의 전제).
struct Retained {
    vx: f32,
    vy: f32,
    s: f32,
    w: u32,
    h: u32,
    exclude: i64,
    rgba: Vec<u8>,
}

/// 다음 프레임에 다시 그릴 영역 로그. 변이(apply/undo/redo) 시 전후 스냅샷 diff가 쌓는다.
struct DamageLog {
    full: bool,
    rects: Vec<(f32, f32, f32, f32)>,
}

impl Default for DamageLog {
    fn default() -> Self {
        Self { full: true, rects: Vec::new() }
    }
}

/// 노드 렌더 시그니처 — 변이 전후 스냅샷을 비교해 손상영역을 만든다(액션 종류 비의존:
/// 새 op가 추가돼도 diff가 잡는다). proxy = 비identity 조상 그룹이 있으면 그 박스
/// (자식 월드 AABB는 조상 트랜스폼 미적용이라 화면 위치가 다르다).
#[derive(PartialEq, Clone)]
struct NodeSig {
    z: u32,
    fields: u64,
    aabb: Option<(f32, f32, f32, f32)>,
    proxy: Option<(f32, f32, f32, f32)>,
}

fn snapshot_sigs(
    doc: &dcli_model::Document,
    extra: dcli_raster::AabbProvider,
) -> std::collections::HashMap<u64, NodeSig> {
    use std::hash::{Hash, Hasher};
    fn walk(
        doc: &dcli_model::Document,
        node: &dcli_model::Node,
        extra: dcli_raster::AabbProvider,
        enclosing: Option<(f32, f32, f32, f32)>,
        z: &mut u32,
        out: &mut std::collections::HashMap<u64, NodeSig>,
    ) {
        use dcli_model::NodeKind;
        *z += 1;
        let aabb = dcli_raster::node_world_aabb_with(doc, node, extra);
        let proxy = enclosing.or(aabb);
        let mut h = std::collections::hash_map::DefaultHasher::new();
        node.visible.hash(&mut h);
        node.opacity.to_bits().hash(&mut h);
        std::mem::discriminant(&node.blend).hash(&mut h);
        node.offset.hash(&mut h);
        (node.scale.0.to_bits(), node.scale.1.to_bits()).hash(&mut h);
        node.rotation.to_bits().hash(&mut h);
        node.meta.hash(&mut h);
        match &node.kind {
            NodeKind::Paint { surface } => {
                surface.0.hash(&mut h);
                // 표면 내용 교체는 새 sid 발급(ReplacePaintSource)이 규약이고, 동일 sid
                // 재기록(트림 restore)은 dims/offset이 함께 변한다 — dims까지 서명에 포함.
                if let Some(s) = doc.pixels().get(*surface) {
                    (s.width(), s.height()).hash(&mut h);
                }
            }
            NodeKind::Group { children } => children.len().hash(&mut h),
        }
        out.insert(
            node.id.0,
            NodeSig { z: *z, fields: h.finish(), aabb, proxy },
        );
        if let NodeKind::Group { children } = &node.kind {
            let child_enc = if node.is_identity_transform() { enclosing } else { enclosing.or(aabb) };
            for cid in children {
                if let Some(c) = doc.get(*cid) {
                    walk(doc, c, extra, child_enc, z, out);
                }
            }
        }
    }
    let mut out = std::collections::HashMap::new();
    let mut z = 0u32;
    for node in doc.iter_bottom_to_top() {
        walk(doc, node, extra, None, &mut z, &mut out);
    }
    out
}

/// rgba 버퍼 내용 시프트: new[x,y] = old[x+ddx, y+ddy] — 행 단위 memmove(겹침 안전).
fn scroll_rgba(buf: &mut [u8], w: i32, h: i32, ddx: i32, ddy: i32) {
    let xs0 = ddx.max(0); // old에서 읽기 시작하는 x
    let xd0 = (-ddx).max(0); // new에 쓰기 시작하는 x
    let cw = (w - ddx.abs()).max(0) as usize;
    if cw == 0 {
        return;
    }
    let rows: Vec<i32> = if ddy >= 0 {
        (0..h).collect() // 앞쪽(아직 안 덮어쓴) 행을 읽는다
    } else {
        (0..h).rev().collect()
    };
    for y in rows {
        let sy = y + ddy;
        if sy < 0 || sy >= h {
            continue;
        }
        let src = ((sy * w + xs0) * 4) as usize;
        let dst = ((y * w + xd0) * 4) as usize;
        buf.copy_within(src..src + cw * 4, dst);
    }
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
            frame: Default::default(),
            damage: Default::default(),
            bounds_cache: Default::default(),
        })
    }

    /// Action 배열 JSON을 트랜잭션으로 적용한다. BatchResult JSON 반환.
    pub fn apply_actions(&mut self, json: &str) -> Result<String, JsError> {
        let actions: Vec<Action> = serde_json::from_str(json)
            .map_err(|e| JsError::new(&format!("Action JSON 파싱: {e}")))?;
        let before = self.begin_mutation();
        let res = dispatch::apply_batch(&mut self.hist, &actions, false);
        if res.ok {
            self.dirty = true;
            self.end_mutation(before);
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
        let before = self.begin_mutation();
        let r = self.hist.undo().map_err(|e| JsError::new(&e.to_string()))?;
        if r {
            self.dirty = true;
            self.end_mutation(before);
        }
        Ok(r)
    }

    pub fn redo(&mut self) -> Result<bool, JsError> {
        let before = self.begin_mutation();
        let r = self.hist.redo().map_err(|e| JsError::new(&e.to_string()))?;
        if r {
            self.dirty = true;
            self.end_mutation(before);
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
    /// 보존 프레임 파이프라인 경유(팬=스크롤, 편집=손상 rect) — 전체 버퍼 복사 반환.
    pub fn composite_view_rgba(
        &self,
        vx: f32,
        vy: f32,
        s: f32,
        w: u32,
        h: u32,
    ) -> js_sys::Uint8ClampedArray {
        let _ = self.render_frame(vx, vy, s, w, h, -1);
        let fr = self.frame.borrow();
        js_sys::Uint8ClampedArray::from(fr.as_ref().map(|f| f.rgba.as_slice()).unwrap_or(&[]))
    }

    /// 뷰 합성 + 노드 1개 화면 제외(텍스트 인라인 편집용). 문서 clone 없이 스킵한다.
    pub fn composite_view_rgba_excluding(
        &self,
        id: u32,
        vx: f32,
        vy: f32,
        s: f32,
        w: u32,
        h: u32,
    ) -> js_sys::Uint8ClampedArray {
        let _ = self.render_frame(vx, vy, s, w, h, id as i32);
        let fr = self.frame.borrow();
        js_sys::Uint8ClampedArray::from(fr.as_ref().map(|f| f.rgba.as_slice()).unwrap_or(&[]))
    }

    /// 보존 프레임 파이프라인 — 바뀐 픽셀만 다시 만들고, 무엇이 바뀌었는지 알려준다.
    ///
    /// 반환(Int32Array): `[mode, ddx, ddy, n, (x,y,w,h)×n]`
    /// - mode 0 = 변화 없음(업로드 불필요)
    /// - mode 1 = 전체 재합성(버퍼 전부 업로드)
    /// - mode 2 = 증분: 캔버스를 (−ddx,−ddy)로 시프트(drawImage)한 뒤 rect n개만
    ///   putImageData로 업로드(노출 스트립 + 편집 손상영역).
    ///
    /// 원점은 디바이스 정수 격자로 양자화(≤0.5px, **표시 전용**) — sub-rect 합성이
    /// 풀프레임과 픽셀 동일해지는 전제다. 버퍼는 frame_pixels()로 제로카피 접근.
    pub fn render_frame(
        &self,
        vx: f32,
        vy: f32,
        s: f32,
        w: u32,
        h: u32,
        exclude: i32,
    ) -> js_sys::Int32Array {
        let doc = &self.hist.doc;
        if s <= 0.0 || w == 0 || h == 0 {
            return js_sys::Int32Array::from(&[0i32, 0, 0, 0][..]);
        }
        let ex_id = if exclude < 0 {
            None
        } else {
            Some(dcli_model::NodeId(exclude as u64))
        };
        let vxq = (vx * s).round() / s;
        let vyq = (vy * s).round() / s;
        let cb = |n: &dcli_model::Node, sc: f32| self.vector_render(doc, n, sc);
        let ab = |n: &dcli_model::Node| self.meta_world_bounds(doc, n);
        let mut dmg = self.damage.borrow_mut();
        let mut fro = self.frame.borrow_mut();

        // 재사용 가능 조건 + 정수 디바이스 델타가 아니면 전체 재합성.
        let mut incremental: Option<(i32, i32)> = None;
        if let Some(f) = fro.as_ref() {
            if f.s == s && f.w == w && f.h == h && f.exclude == exclude as i64 && !dmg.full {
                let ddxf = (vxq - f.vx) * s;
                let ddyf = (vyq - f.vy) * s;
                let (ddx, ddy) = (ddxf.round() as i32, ddyf.round() as i32);
                if (ddxf - ddx as f32).abs() < 1e-2
                    && (ddyf - ddy as f32).abs() < 1e-2
                    && ddx.unsigned_abs() < w
                    && ddy.unsigned_abs() < h
                {
                    incremental = Some((ddx, ddy));
                }
            }
        }
        let Some((ddx, ddy)) = incremental else {
            let sfc = dcli_raster::composite_view_display(doc, vxq, vyq, s, w, h, ex_id, &ab, &cb);
            let rgba = sfc.to_srgb8_rgba_fast();
            *fro = Some(Retained {
                vx: vxq,
                vy: vyq,
                s,
                w,
                h,
                exclude: exclude as i64,
                rgba,
            });
            dmg.full = false;
            dmg.rects.clear();
            return js_sys::Int32Array::from(&[1i32, 0, 0, 0][..]);
        };

        let f = fro.as_mut().expect("incremental은 frame 존재가 전제");
        let (wi, hi) = (w as i32, h as i32);
        let mut rects: Vec<(i32, i32, i32, i32)> = Vec::new();
        if ddx != 0 || ddy != 0 {
            scroll_rgba(&mut f.rgba, wi, hi, ddx, ddy);
            f.vx = vxq;
            f.vy = vyq;
            // 노출 스트립(세로 + 가로, 코너 중복은 무해).
            if ddx > 0 {
                rects.push((wi - ddx, 0, ddx, hi));
            } else if ddx < 0 {
                rects.push((0, 0, -ddx, hi));
            }
            if ddy > 0 {
                rects.push((0, hi - ddy, wi, ddy));
            } else if ddy < 0 {
                rects.push((0, 0, wi, -ddy));
            }
        }
        // 편집 손상영역(월드) → 디바이스 rect(±4px: 컬링/재래스터 마진과 동일).
        let pad = 4i32;
        let mut drects: Vec<(i32, i32, i32, i32)> = Vec::new();
        for r in dmg.rects.drain(..) {
            let x0 = (((r.0 - f.vx) * s).floor() as i32 - pad).clamp(0, wi);
            let y0 = (((r.1 - f.vy) * s).floor() as i32 - pad).clamp(0, hi);
            let x1 = (((r.2 - f.vx) * s).ceil() as i32 + pad).clamp(0, wi);
            let y1 = (((r.3 - f.vy) * s).ceil() as i32 + pad).clamp(0, hi);
            if x1 > x0 && y1 > y0 {
                drects.push((x0, y0, x1 - x0, y1 - y0));
            }
        }
        if drects.len() > 8 {
            // 너무 잘게 쪼개졌으면 합집합 1개로(putImageData 횟수 제한).
            let u = drects.iter().fold((wi, hi, 0, 0), |a, r| {
                (a.0.min(r.0), a.1.min(r.1), a.2.max(r.0 + r.2), a.3.max(r.1 + r.3))
            });
            drects = vec![(u.0, u.1, u.2 - u.0, u.3 - u.1)];
        }
        rects.extend(drects);
        // 겹침 병합(낭비 제한): 같은 영역 중복 재합성 방지(전후 box 동일한 props 편집이
        // rect 2개를 만드는 케이스). 합집합이 면적 합의 1.3배 이하일 때만 합친다 —
        // 가는 스트립 + 작은 손상 rect를 거대한 union으로 만들지 않는다.
        let mut merged: Vec<(i32, i32, i32, i32)> = Vec::new();
        for r in rects {
            let mut cur = r;
            loop {
                let mut absorbed = false;
                merged.retain(|m| {
                    let overlap = cur.0 < m.0 + m.2
                        && m.0 < cur.0 + cur.2
                        && cur.1 < m.1 + m.3
                        && m.1 < cur.1 + cur.3;
                    if !overlap {
                        return true;
                    }
                    let ux0 = cur.0.min(m.0);
                    let uy0 = cur.1.min(m.1);
                    let ux1 = (cur.0 + cur.2).max(m.0 + m.2);
                    let uy1 = (cur.1 + cur.3).max(m.1 + m.3);
                    let ua = (ux1 - ux0) as i64 * (uy1 - uy0) as i64;
                    let sum = cur.2 as i64 * cur.3 as i64 + m.2 as i64 * m.3 as i64;
                    if ua * 10 <= sum * 13 {
                        cur = (ux0, uy0, ux1 - ux0, uy1 - uy0);
                        absorbed = true;
                        false // 흡수된 기존 rect 제거
                    } else {
                        true
                    }
                });
                if !absorbed {
                    break;
                }
            }
            merged.push(cur);
        }
        let rects = merged;
        if rects.is_empty() {
            return js_sys::Int32Array::from(&[0i32, 0, 0, 0][..]);
        }
        for &(rx, ry, rw, rh) in &rects {
            let sub = dcli_raster::composite_view_display(
                doc,
                f.vx + rx as f32 / s,
                f.vy + ry as f32 / s,
                s,
                rw as u32,
                rh as u32,
                ex_id,
                &ab,
                &cb,
            );
            let bytes = sub.to_srgb8_rgba_fast();
            let rww = rw as usize * 4;
            for row in 0..rh as usize {
                let dst = ((ry as usize + row) * w as usize + rx as usize) * 4;
                f.rgba[dst..dst + rww].copy_from_slice(&bytes[row * rww..(row + 1) * rww]);
            }
        }
        let mut out = vec![2i32, ddx, ddy, rects.len() as i32];
        for r in &rects {
            out.extend_from_slice(&[r.0, r.1, r.2, r.3]);
        }
        js_sys::Int32Array::from(&out[..])
    }

    /// 보존 프레임 버퍼(sRGB8 RGBA, w×h×4)의 **제로카피 뷰**.
    ///
    /// 계약: 반환 직후 putImageData까지 다른 wasm 호출 금지 — wasm 메모리가 성장하면
    /// 뷰의 ArrayBuffer가 detach된다. 프레임이 없으면 길이 0.
    pub fn frame_pixels(&self) -> js_sys::Uint8ClampedArray {
        match &*self.frame.borrow() {
            Some(f) => {
                // Uint8ClampedArray::view는 wasm-bindgen cast intrinsic을 타며 실제로는
                // Uint8Array 브랜드를 반환한다(ImageData 생성자가 거부 → 캔버스 블랭크).
                // 같은 메모리 범위 위에 진짜 Uint8ClampedArray를 직접 생성한다(여전히 제로카피).
                let v = unsafe { js_sys::Uint8Array::view(&f.rgba) };
                js_sys::Uint8ClampedArray::new_with_byte_offset_and_length(
                    &v.buffer(),
                    v.byte_offset(),
                    v.length(),
                )
            }
            None => js_sys::Uint8ClampedArray::new_with_length(0),
        }
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
        // 브라우저 표시와 같은 디스플레이 블렌드 경로(시각 검수 산출물의 대표성).
        let ab = |n: &dcli_model::Node| self.meta_world_bounds(doc, n);
        let rgba = dcli_raster::composite_view_display(doc, vx, vy, s, w, h, None, &ab, &|n, sc| {
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
        self.damage.borrow_mut().full = true; // 글꼴 교체 = 텍스트 전부 재래스터.
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
            frame: Default::default(),
            damage: Default::default(),
            bounds_cache: Default::default(),
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
    /// identity Paint 노드의 meta 벡터 아이템 **월드 경계**(그림자/스트로크 마진 포함).
    ///
    /// 컬링·그룹 tmp·손상영역이 표면 밖으로 뻗은 meta(set_props meta-only 흐름)를
    /// 놓치지 않게 한다. (meta+offset 해시) 키로 캐시 — 파싱은 변경 시에만.
    fn meta_world_bounds(
        &self,
        doc: &dcli_model::Document,
        node: &dcli_model::Node,
    ) -> Option<(f32, f32, f32, f32)> {
        use std::hash::{Hash, Hasher};
        node.meta.as_deref()?;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        node.meta.hash(&mut h);
        node.offset.hash(&mut h);
        let key = h.finish();
        if let Some(v) = self.bounds_cache.borrow().get(&key) {
            return *v;
        }
        let b = vector_items_of(doc, node)
            .as_deref()
            .and_then(dcli_raster::view_items_bounds);
        let mut cache = self.bounds_cache.borrow_mut();
        if cache.len() > 1024 {
            cache.clear();
        }
        cache.insert(key, b);
        b
    }

    /// 변이 직전 호출 — diff용 사전 스냅샷. 이미 full 손상이면 스냅샷 생략(None).
    fn begin_mutation(
        &self,
    ) -> Option<(std::collections::HashMap<u64, NodeSig>, (u32, u32))> {
        if self.damage.borrow().full {
            return None;
        }
        let doc = &self.hist.doc;
        let sigs = snapshot_sigs(doc, &|n| self.meta_world_bounds(doc, n));
        Some((sigs, (doc.width, doc.height)))
    }

    /// 변이 직후 호출 — 전후 시그니처 diff로 손상 rect를 누적한다.
    fn end_mutation(
        &self,
        before: Option<(std::collections::HashMap<u64, NodeSig>, (u32, u32))>,
    ) {
        let mut dmg = self.damage.borrow_mut();
        if dmg.full {
            return;
        }
        let Some((before, bdims)) = before else {
            dmg.full = true;
            dmg.rects.clear();
            return;
        };
        let doc = &self.hist.doc;
        if bdims != (doc.width, doc.height) {
            dmg.full = true;
            dmg.rects.clear();
            return;
        }
        let after = snapshot_sigs(doc, &|n| self.meta_world_bounds(doc, n));
        for (id, b) in &before {
            match after.get(id) {
                None => {
                    if let Some(r) = b.proxy {
                        dmg.rects.push(r);
                    }
                }
                Some(a) if a != b => {
                    if let Some(r) = b.proxy {
                        dmg.rects.push(r);
                    }
                    if let Some(r) = a.proxy {
                        dmg.rects.push(r);
                    }
                }
                _ => {}
            }
        }
        for (id, a) in &after {
            if !before.contains_key(id) {
                if let Some(r) = a.proxy {
                    dmg.rects.push(r);
                }
            }
        }
        // 폭주 방지: 변이 폭이 크면(에이전트 대량 배치) 전체 재합성이 더 싸다.
        if dmg.rects.len() > 32 {
            dmg.full = true;
            dmg.rects.clear();
        }
    }

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

    /// 뷰 배율 레이어 캐시 — **전 배율**. 벡터 meta는 재래스터(확대 계단 제거, 축소
    /// 타깃 해상도 AA), 비벡터(이미지) 레이어는 스케일 리샘플 1회 → 이후 정수 블릿
    /// (매 프레임 픽셀당 bilinear/슈퍼샘플 제거 — PSD/사진 문서의 지배 비용).
    /// 키 = (표면 id, meta+offset 해시, scale bits): 편집(표면 교체)·이동·meta·줌
    /// 변경 시 자연 무효화, 같은 줌의 팬·재합성은 블릿만.
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
        let mut hsh = std::collections::hash_map::DefaultHasher::new();
        node.meta.hash(&mut hsh);
        node.offset.hash(&mut hsh); // 이동 시 월드 좌표(리샘플 위상)가 바뀌므로 키에 포함.
        let key = (surface.0, hsh.finish(), s.to_bits());
        if let Some(hit) = self.view_cache.borrow().get(&key) {
            return Some(hit.clone());
        }
        let entry = if let Some(items) = vector_items_of(doc, node) {
            let (sfc, origin) = dcli_raster::render_view_items(&items, s, 16_000_000)?;
            (std::rc::Rc::new(sfc), origin)
        } else {
            // 이미지(비벡터) 레이어: composite_layer_view와 동일한 샘플 수학으로 뷰
            // 배율 표면을 만들어 둔다. s=1은 정수 시프트 직행 경로가 있으므로 제외.
            if (s - 1.0).abs() < 1e-6 {
                return None;
            }
            // 고수위 가드: 캐시가 이미 가득이면 라이브 샘플 폴백(뷰포트 비례 비용).
            // clear() 재삽입 스래시(매 프레임 전 레이어 리샘플)가 라이브보다 나쁘다.
            let total_px: u64 = self
                .view_cache
                .borrow()
                .values()
                .map(|(s, _)| s.width() as u64 * s.height() as u64)
                .sum();
            if total_px > 24_000_000 {
                return None;
            }
            let src = doc.pixels().get(surface)?;
            let (sfc, origin) = scale_raster_for_view(src, node.offset, s, 16_000_000)?;
            (std::rc::Rc::new(sfc), origin)
        };
        let mut cache = self.view_cache.borrow_mut();
        // 예산: 엔트리 수 + 총 픽셀(16B/px — 큰 표면 몇 장이면 수백 MB). 초과 시 전체
        // 비움(단순·안전): 다음 프레임 가시 항목만 다시 채워져 working set으로 수렴한다.
        let new_px = entry.0.width() as u64 * entry.0.height() as u64;
        let total_px: u64 = cache
            .values()
            .map(|(s, _)| s.width() as u64 * s.height() as u64)
            .sum();
        if cache.len() > 128 || total_px + new_px > 32_000_000 {
            cache.clear();
        }
        cache.insert(key, entry.clone());
        Some(entry)
    }
}

/// 이미지(비벡터) 레이어를 뷰 배율 s로 리샘플한 표면 + 디바이스 좌표 원점(월드×s).
///
/// composite_layer_view(identity)와 동일한 샘플 수학(픽셀중심 bilinear, s<0.75는
/// 2×2 슈퍼샘플, 격자 밖 투명)을 위상(offset×s의 소수부)까지 그대로 굽는다 —
/// 뷰 원점이 디바이스 정수 격자에 있을 때(setView 스냅/프레임 양자화) 라이브 샘플과
/// 일치한다. 결과 픽셀 수가 max_px를 넘으면 None(라이브 경로 폴백 — 뷰포트가 잘라줌).
fn scale_raster_for_view(
    src: &dcli_tile::Surface,
    offset: (i32, i32),
    s: f32,
    max_px: u64,
) -> Option<(dcli_tile::Surface, (i32, i32))> {
    use dcli_color::LinearPremul;
    if s <= 0.0 {
        return None;
    }
    let (sw, sh) = (src.width() as i32, src.height() as i32);
    let (ox, oy) = (offset.0 as f32, offset.1 as f32);
    // 디바이스 그리드 범위(+1px 필터 서포트 마진).
    let x0 = (ox * s).floor() as i32 - 1;
    let y0 = (oy * s).floor() as i32 - 1;
    let x1 = ((ox + sw as f32) * s).ceil() as i32 + 1;
    let y1 = ((oy + sh as f32) * s).ceil() as i32 + 1;
    let w = (x1 - x0).max(1) as u32;
    let h = (y1 - y0).max(1) as u32;
    if w as u64 * h as u64 > max_px {
        return None;
    }
    let px = src.pixels();
    let zero = LinearPremul { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    let tap = |ix: i32, iy: i32| -> LinearPremul {
        if ix < 0 || iy < 0 || ix >= sw || iy >= sh {
            zero
        } else {
            px[(iy * sw + ix) as usize]
        }
    };
    let lerp = |a: LinearPremul, b: LinearPremul, t: f32| LinearPremul {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    };
    let inv = 1.0 / s;
    let sample_at = |dx: f32, dy: f32| -> LinearPremul {
        let fx = dx * inv - ox - 0.5;
        let fy = dy * inv - oy - 0.5;
        let ix = fx.floor() as i32;
        let iy = fy.floor() as i32;
        let tx = fx - ix as f32;
        let ty = fy - iy as f32;
        lerp(
            lerp(tap(ix, iy), tap(ix + 1, iy), tx),
            lerp(tap(ix, iy + 1), tap(ix + 1, iy + 1), tx),
            ty,
        )
    };
    let supersample = s < 0.75;
    let mut out = dcli_tile::Surface::new(w, h);
    let dst = out.pixels_mut();
    for j in 0..h as i32 {
        for i in 0..w as i32 {
            let cx = (x0 + i) as f32;
            let cy = (y0 + j) as f32;
            let v = if supersample {
                let a = sample_at(cx + 0.25, cy + 0.25);
                let b = sample_at(cx + 0.75, cy + 0.25);
                let c = sample_at(cx + 0.25, cy + 0.75);
                let d = sample_at(cx + 0.75, cy + 0.75);
                LinearPremul {
                    r: (a.r + b.r + c.r + d.r) * 0.25,
                    g: (a.g + b.g + c.g + d.g) * 0.25,
                    b: (a.b + b.b + c.b + d.b) * 0.25,
                    a: (a.a + b.a + c.a + d.a) * 0.25,
                }
            } else {
                sample_at(cx + 0.5, cy + 0.5)
            };
            dst[(j as u32 * w + i as u32) as usize] = v;
        }
    }
    Some((out, (x0, y0)))
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
