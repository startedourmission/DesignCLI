//! 편집 디스패치 엔진 — CLI와 MCP가 공유하는 단일 쓰기 경로.
//!
//! `Action`은 에이전트/CLI가 보내는 고수준 편집 명령이다. 코어 `Op`과 달리:
//! - `NodeRef`로 **아직 발급 안 된 노드**를 batch 내 named binding으로 참조할 수 있고,
//! - `PixelSource`로 픽셀을 여러 출처(투명/단색/PNG/기존표면)에서 가져온다(IO·decode 경계).
//!
//! `apply_batch`가 Action 배열을 트랜잭션으로 실행한다: 전부 성공하면 batch 전체가
//! undo 1단위로 commit, 하나라도 실패하면 **문서·픽셀스토어를 비트 단위 원복**하고
//! 고칠 이슈 목록을 반환한다(검증 #1 orphan surface 회수 포함).

use anyhow::Result;
use dcli_color::LinearPremul;
use dcli_model::{BlendMode, History, NodeId, NodeProps, Op};
use dcli_tile::{Surface, SurfaceId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(feature = "fs-sources")]
use std::path::PathBuf;

/// 노드 참조: 발급된 id 또는 같은 batch 내 named binding.
/// 명시 태그({"node":5} | {"bind":"x"})로 검증 실패 메시지를 명확히 한다(검증 #2).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeRef {
    /// 이미 발급된 노드 id.
    Node(u64),
    /// 같은 batch에서 bind된 이름.
    Bind(String),
}

/// 새 페인트 레이어의 픽셀 출처.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum PixelSource {
    /// 완전 투명.
    Transparent,
    /// 단색 채우기 (straight sRGB8 RGBA).
    Fill { rgba: [u8; 4] },
    /// base64 인코딩된 PNG(8bit RGBA).
    PngBase64 { data: String },
    /// 디스크의 PNG 경로. (fs-sources 전용 — wasm 빌드에는 없음)
    #[cfg(feature = "fs-sources")]
    PngPath { path: PathBuf },
    /// 투명 위에 도형들을 순서대로 그린다(안티에일리어싱).
    Shapes { items: Vec<Shape> },
}

/// 그릴 도형 하나(좌표는 픽셀 단위 f32).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum Shape {
    /// 채워진 사각형: 좌상단 (x,y), 크기 (w,h).
    Rect { x: f32, y: f32, w: f32, h: f32, rgba: [u8; 4] },
    /// 채워진 타원: 중심 (cx,cy), 반지름 (rx,ry).
    Ellipse { cx: f32, cy: f32, rx: f32, ry: f32, rgba: [u8; 4] },
    /// 선분: (x0,y0)→(x1,y1), 두께 width.
    Line { x0: f32, y0: f32, x1: f32, y1: f32, width: f32, rgba: [u8; 4] },
}

/// 노드 속성 부분 패치(지정한 필드만 변경).
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct PropPatch {
    pub name: Option<String>,
    pub visible: Option<bool>,
    pub opacity: Option<f32>,
}

/// 블렌드 모드(문자열 직렬화).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlendModeDto {
    Normal,
    Multiply,
    Screen,
}

impl From<BlendModeDto> for BlendMode {
    fn from(b: BlendModeDto) -> Self {
        match b {
            BlendModeDto::Normal => BlendMode::Normal,
            BlendModeDto::Multiply => BlendMode::Multiply,
            BlendModeDto::Screen => BlendMode::Screen,
        }
    }
}

/// 고수준 편집 명령. batch 또는 단발로 실행된다.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Action {
    /// 페인트 레이어 추가. `bind`로 후속 action이 이 노드를 참조할 수 있다.
    AddPaintLayer {
        #[serde(default = "default_layer_name")]
        name: String,
        source: PixelSource,
        #[serde(default)]
        index: Option<usize>,
        /// 같은 batch 내 참조용 이름(서버가 발급한 id에 바인딩).
        #[serde(default)]
        bind: Option<String>,
    },
    /// 레이어 삭제.
    DeleteLayer { id: NodeRef },
    /// 레이어를 새 순서 인덱스로 이동(bottom-to-top).
    MoveLayer { id: NodeRef, to: usize },
    /// 노드 속성 부분 변경.
    SetProps { id: NodeRef, patch: PropPatch },
    /// 블렌드 모드 변경.
    SetBlend { id: NodeRef, mode: BlendModeDto },
}

fn default_layer_name() -> String {
    "layer".to_string()
}

/// batch 실행 이슈(self-correction용 구조화 에러).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Issue {
    pub op_index: usize,
    pub op_kind: String,
    pub code: String,
    pub message: String,
}

/// batch 결과.
#[derive(Debug, Clone, Serialize)]
pub struct BatchResult {
    pub ok: bool,
    pub applied: usize,
    /// 성공 시 bind 이름 → 발급된 노드/표면 id.
    pub bindings: HashMap<String, BindingOut>,
    pub issues: Vec<Issue>,
    /// 실패해서 중단된 action 인덱스(있으면).
    pub aborted_at: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BindingOut {
    pub node: u64,
    pub surface: Option<u64>,
}

/// batch 내 bind 이름 → 발급된 (노드, 표면).
#[derive(Clone, Copy)]
struct Binding {
    node: NodeId,
    surface: Option<SurfaceId>,
}

/// straight sRGB8 PNG 바이트를 문서 크기 표면(linear-premul)으로 디코드.
fn decode_png(bytes: &[u8], w: u32, h: u32) -> Result<Surface, String> {
    let dec = png::Decoder::new(bytes);
    let mut reader = dec.read_info().map_err(|e| format!("PNG 헤더 오류: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("PNG 디코드 오류: {e}"))?;
    if info.width != w || info.height != h {
        return Err(format!(
            "이미지 크기 {}x{}가 문서 {}x{}와 불일치",
            info.width, info.height, w, h
        ));
    }
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err("8bit RGBA PNG만 지원".to_string());
    }
    let mut s = Surface::new(w, h);
    for (i, px) in buf[..info.buffer_size()].chunks_exact(4).enumerate() {
        let x = i as u32 % w;
        let y = i as u32 / w;
        s.set(x, y, LinearPremul::from_srgb8_straight(px[0], px[1], px[2], px[3]));
    }
    Ok(s)
}

/// PixelSource를 표면으로 materialize.
fn materialize(source: &PixelSource, w: u32, h: u32) -> Result<Surface, String> {
    match source {
        PixelSource::Transparent => Ok(Surface::new(w, h)),
        PixelSource::Fill { rgba } => Ok(Surface::filled(
            w,
            h,
            LinearPremul::from_srgb8_straight(rgba[0], rgba[1], rgba[2], rgba[3]),
        )),
        PixelSource::PngBase64 { data } => {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data.trim())
                .map_err(|e| format!("base64 디코드 오류: {e}"))?;
            decode_png(&bytes, w, h)
        }
        #[cfg(feature = "fs-sources")]
        PixelSource::PngPath { path } => {
            let bytes = std::fs::read(path).map_err(|e| format!("이미지 읽기 실패: {e}"))?;
            decode_png(&bytes, w, h)
        }
        PixelSource::Shapes { items } => {
            let mut s = Surface::new(w, h);
            for shape in items {
                draw_shape(&mut s, shape);
            }
            Ok(s)
        }
    }
}

/// 한 도형을 표면에 그린다(dcli-raster::shapes 위임).
fn draw_shape(s: &mut Surface, shape: &Shape) {
    use dcli_raster::shapes;
    match *shape {
        Shape::Rect { x, y, w, h, rgba } => shapes::fill_rect(s, x, y, w, h, rgba),
        Shape::Ellipse { cx, cy, rx, ry, rgba } => shapes::fill_ellipse(s, cx, cy, rx, ry, rgba),
        Shape::Line { x0, y0, x1, y1, width, rgba } => {
            shapes::stroke_line(s, x0, y0, x1, y1, width, rgba)
        }
    }
}

/// NodeRef를 실제 NodeId로 해소(bind는 binder에서 찾는다).
fn resolve_ref(r: &NodeRef, binder: &HashMap<String, Binding>) -> Result<NodeId, (String, String)> {
    match r {
        NodeRef::Node(id) => Ok(NodeId(*id)),
        NodeRef::Bind(name) => binder
            .get(name)
            .map(|b| b.node)
            .ok_or_else(|| ("unresolved_ref".to_string(), format!("bind 이름 '{name}' 미해결"))),
    }
}

fn action_kind(a: &Action) -> &'static str {
    match a {
        Action::AddPaintLayer { .. } => "add_paint_layer",
        Action::DeleteLayer { .. } => "delete_layer",
        Action::MoveLayer { .. } => "move_layer",
        Action::SetProps { .. } => "set_props",
        Action::SetBlend { .. } => "set_blend",
    }
}

/// Action 배열을 트랜잭션으로 적용한다.
///
/// 성공: batch 전체를 commit(undo 1단위), `BatchResult{ok:true, bindings}`.
/// 실패: 전체 롤백 + orphan 표면 회수, `BatchResult{ok:false, issues, aborted_at}`.
/// `dry_run`: 검증만 하고 항상 롤백(문서·픽셀 무변경).
pub fn apply_batch(h: &mut History, actions: &[Action], dry_run: bool) -> BatchResult {
    let (w, hh) = (h.doc.width, h.doc.height);
    let sp = h.savepoint();
    let mut binder: HashMap<String, Binding> = HashMap::new();
    // batch가 PixelStore에 등록한 표면(롤백 시 회수 대상).
    let mut owned: Vec<SurfaceId> = Vec::new();
    let mut issues = Vec::new();
    let mut applied = 0usize;
    let mut aborted_at = None;

    for (i, action) in actions.iter().enumerate() {
        match try_one(h, action, &mut binder, &mut owned, w, hh) {
            Ok(()) => applied += 1,
            Err((code, message)) => {
                issues.push(Issue {
                    op_index: i,
                    op_kind: action_kind(action).to_string(),
                    code,
                    message,
                });
                aborted_at = Some(i);
                break;
            }
        }
    }

    let success = issues.is_empty();
    if success && !dry_run {
        h.commit_batch(sp);
        BatchResult {
            ok: true,
            applied,
            bindings: binder
                .into_iter()
                .map(|(k, v)| {
                    (k, BindingOut { node: v.node.0, surface: v.surface.map(|s| s.0) })
                })
                .collect(),
            issues,
            aborted_at: None,
        }
    } else {
        // 롤백 + orphan 표면 회수. (실패했거나 dry_run인 경우)
        let _ = h.rollback_to(sp);
        for sid in owned.iter().rev() {
            h.doc.pixels_mut().remove(*sid);
        }
        BatchResult {
            ok: success, // dry_run 성공이면 ok:true(검증 통과)지만 applied는 롤백됨.
            applied: if success { applied } else { 0 },
            // dry_run 응답은 실제 id를 약속하지 않는다(검증 #2b): bindings 비움.
            bindings: HashMap::new(),
            issues,
            aborted_at,
        }
    }
}

/// 한 Action을 stage한다. 실패 시 (code, message).
fn try_one(
    h: &mut History,
    action: &Action,
    binder: &mut HashMap<String, Binding>,
    owned: &mut Vec<SurfaceId>,
    w: u32,
    hh: u32,
) -> Result<(), (String, String)> {
    match action {
        Action::AddPaintLayer { name, source, index, bind } => {
            let surface = materialize(source, w, hh).map_err(|m| ("bad_surface_source".to_string(), m))?;
            let sid = h.doc.add_surface(surface);
            owned.push(sid); // 롤백 시 회수 대상으로 추적.
            h.stage(Op::AddPaintLayer { name: name.clone(), surface: sid, index: *index, forced_id: None })
                .map_err(op_err)?;
            let node = *h.doc.order().last().expect("방금 추가됨");
            if let Some(b) = bind {
                if binder.contains_key(b) {
                    return Err(("duplicate_bind".to_string(), format!("bind 이름 '{b}' 중복")));
                }
                binder.insert(b.clone(), Binding { node, surface: Some(sid) });
            }
            Ok(())
        }
        Action::DeleteLayer { id } => {
            let nid = resolve_ref(id, binder)?;
            h.stage(Op::DeleteLayer { id: nid }).map_err(op_err)
        }
        Action::MoveLayer { id, to } => {
            let nid = resolve_ref(id, binder)?;
            h.stage(Op::MoveLayer { id: nid, to: *to }).map_err(op_err)
        }
        Action::SetProps { id, patch } => {
            let nid = resolve_ref(id, binder)?;
            let node = h.doc.get(nid).ok_or_else(|| {
                ("node_not_found".to_string(), format!("노드 n{} 없음", nid.0))
            })?;
            let mut props = NodeProps::of(node);
            if let Some(n) = &patch.name {
                props.name = n.clone();
            }
            if let Some(v) = patch.visible {
                props.visible = v;
            }
            if let Some(o) = patch.opacity {
                props.opacity = o;
            }
            h.stage(Op::SetProps { id: nid, props }).map_err(op_err)
        }
        Action::SetBlend { id, mode } => {
            let nid = resolve_ref(id, binder)?;
            let node = h.doc.get(nid).ok_or_else(|| {
                ("node_not_found".to_string(), format!("노드 n{} 없음", nid.0))
            })?;
            let props = NodeProps { blend: (*mode).into(), ..NodeProps::of(node) };
            h.stage(Op::SetProps { id: nid, props }).map_err(op_err)
        }
    }
}

/// OpError를 (code, message)로 매핑.
fn op_err(e: dcli_model::OpError) -> (String, String) {
    use dcli_model::OpError::*;
    let code = match e {
        NodeNotFound(_) => "node_not_found",
        SurfaceNotFound(_) => "surface_not_found",
    };
    (code.to_string(), e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcli_color::BitDepth;
    use dcli_model::Document;

    fn doc() -> Document {
        Document::new(8, 8, BitDepth::U8)
    }

    fn fill(r: u8, g: u8, b: u8, a: u8) -> PixelSource {
        PixelSource::Fill { rgba: [r, g, b, a] }
    }

    #[test]
    fn batch_add_with_named_binding() {
        let mut h = History::new(doc());
        let actions = vec![
            Action::AddPaintLayer { name: "bg".into(), source: fill(255, 0, 0, 255), index: None, bind: Some("bg".into()) },
            // 같은 batch에서 방금 만든 노드를 bind로 참조.
            Action::SetBlend { id: NodeRef::Bind("bg".into()), mode: BlendModeDto::Multiply },
        ];
        let res = apply_batch(&mut h, &actions, false);
        assert!(res.ok, "issues: {:?}", res.issues);
        assert_eq!(res.applied, 2);
        assert!(res.bindings.contains_key("bg"));
        let id = h.doc.order()[0];
        assert_eq!(h.doc.get(id).unwrap().blend, BlendMode::Multiply);
    }

    #[test]
    fn batch_rollback_no_orphan_surface() {
        // 검증 #1 회귀: add 성공 후 후속 op 실패 → 전체 롤백 + 표면 회수.
        let mut h = History::new(doc());
        let before_surfaces = h.doc.pixels().len();
        let actions = vec![
            Action::AddPaintLayer { name: "a".into(), source: fill(1, 2, 3, 255), index: None, bind: None },
            Action::AddPaintLayer { name: "b".into(), source: fill(4, 5, 6, 255), index: None, bind: None },
            // 존재하지 않는 노드 참조 → 실패.
            Action::DeleteLayer { id: NodeRef::Node(999) },
        ];
        let res = apply_batch(&mut h, &actions, false);
        assert!(!res.ok);
        assert_eq!(res.aborted_at, Some(2));
        assert_eq!(res.applied, 0, "실패 batch는 applied=0");
        // ★표면 누수 0★: 등록했던 2개 표면이 모두 회수됨.
        assert_eq!(h.doc.pixels().len(), before_surfaces, "orphan 표면 누수");
        assert_eq!(h.doc.node_count(), 0, "노드도 원복");
    }

    #[test]
    fn unresolved_bind_fails() {
        let mut h = History::new(doc());
        let actions = vec![Action::DeleteLayer { id: NodeRef::Bind("nope".into()) }];
        let res = apply_batch(&mut h, &actions, false);
        assert!(!res.ok);
        assert_eq!(res.issues[0].code, "unresolved_ref");
    }

    #[test]
    fn dry_run_makes_no_change() {
        let mut h = History::new(doc());
        let actions = vec![Action::AddPaintLayer {
            name: "x".into(),
            source: fill(0, 0, 0, 255),
            index: None,
            bind: None,
        }];
        let res = apply_batch(&mut h, &actions, true);
        assert!(res.ok, "검증 통과해야");
        assert_eq!(h.doc.node_count(), 0, "dry_run은 무변경");
        assert_eq!(h.doc.pixels().len(), 0, "dry_run은 표면도 무변경");
        assert!(res.bindings.is_empty(), "dry_run은 실제 id 약속 안 함");
    }

    #[test]
    fn shapes_layer_draws_pixels() {
        // 도형 레이어가 실제 픽셀을 그리는지(투명 위 빨간 사각형).
        let mut h = History::new(Document::new(16, 16, BitDepth::U8));
        let actions = vec![Action::AddPaintLayer {
            name: "shapes".into(),
            source: PixelSource::Shapes {
                items: vec![Shape::Rect { x: 4.0, y: 4.0, w: 8.0, h: 8.0, rgba: [255, 0, 0, 255] }],
            },
            index: None,
            bind: None,
        }];
        let res = apply_batch(&mut h, &actions, false);
        assert!(res.ok, "issues: {:?}", res.issues);
        // 합성해서 사각형 내부가 빨강인지 확인.
        let out = dcli_raster::composite(&h.doc).to_srgb8_rgba();
        let idx = ((8 * 16) + 8) * 4; // (8,8) 픽셀.
        assert_eq!(&out[idx..idx + 4], &[255, 0, 0, 255], "사각형 내부 빨강");
        // 바깥(0,0)은 투명.
        assert_eq!(&out[0..4], &[0, 0, 0, 0], "바깥 투명");
    }

    #[test]
    fn duplicate_bind_rejected() {
        let mut h = History::new(doc());
        let actions = vec![
            Action::AddPaintLayer { name: "a".into(), source: fill(1, 1, 1, 255), index: None, bind: Some("dup".into()) },
            Action::AddPaintLayer { name: "b".into(), source: fill(2, 2, 2, 255), index: None, bind: Some("dup".into()) },
        ];
        let res = apply_batch(&mut h, &actions, false);
        assert!(!res.ok);
        assert_eq!(res.issues[0].code, "duplicate_bind");
        assert_eq!(h.doc.pixels().len(), 0, "롤백 후 표면 누수 0");
    }
}
