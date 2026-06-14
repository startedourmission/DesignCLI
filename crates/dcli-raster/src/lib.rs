//! CPU 정본(oracle) 합성기.
//!
//! 이것이 export·골든이미지·CLI의 **결정적 정본**이다(architecture-decision).
//! GPU(`dcli-gpu`)는 이 결과를 SSIM/max-abs ~1e-4 허용오차 내에서 재현하는
//! 가속 프리뷰일 뿐이며, export 비트를 만들지 않는다.
//!
//! ★최대 위험★ (gamma-vs-linear-landmine): 합성 색공간이 비트깊이로 분기한다.
//! - `BlendSpace::Gamma` (8/16bit, Photoshop 기본): 블렌드 수학을 **감마 인코딩된**
//!   sRGB 값에 직접 적용. 내부 저장은 linear-premul이므로 블렌드 직전에 감마로
//!   되돌리고(per-component OETF), 블렌드 후 다시 선형화.
//! - `BlendSpace::Linear` (32bit/HDR): linear-light에서 직접 블렌드.
//!
//! 이 둘은 같은 함수의 분기가 아니라 의미가 다른 두 경로다. 같은 입력이라도
//! 결과가 다르며, 그 차이가 Photoshop 일치 여부를 가른다.

#![forbid(unsafe_code)]

pub mod shapes;
pub mod text;
#[cfg(not(target_arch = "wasm32"))]
pub mod sysfonts;

use dcli_color::{srgb_eotf, srgb_eotf_fast, srgb_oetf, srgb_oetf_fast, BlendSpace, LinearPremul};
use dcli_model::{BlendMode, Document};
use dcli_tile::Surface;

/// 문서를 한 장의 표면으로 합성한다 (CPU 정본). = composite_region(0,0,w,h).
pub fn composite(doc: &Document) -> Surface {
    composite_region(doc, 0, 0, doc.width, doc.height)
}

/// 문서의 **임의 영역**을 합성한다 — 무한 작업영역/Frame export의 토대.
/// (rx, ry)가 출력 (0,0)에 오는 rw×rh 표면을 만든다. 영역 밖 내용은 잘린다.
pub fn composite_region(doc: &Document, rx: i32, ry: i32, rw: u32, rh: u32) -> Surface {
    let mut acc = Surface::new(rw, rh);
    for node in doc.iter_bottom_to_top() {
        composite_node(&mut acc, doc, node, (rx, ry), 0);
    }
    acc
}

/// 화면(뷰) 합성 — **보이는 영역만 출력 해상도로 직접 샘플**한다.
///
/// 합성 비용이 장면 크기가 아니라 출력 픽셀 수에 비례한다(Figma식 "보이는 것만 그린다").
/// 출력 (0,0)의 월드 좌표 = (vx, vy), 출력 1px = 1/s 문서 px (s = 줌 × 렌더스케일).
/// s == 1이고 (vx, vy)가 정수면 identity 레이어는 composite_region과 **비트 동일**
/// (정수 시프트 경로) — 디바이스 100% 뷰가 자동으로 픽셀 퍼펙트가 된다.
pub fn composite_view(doc: &Document, vx: f32, vy: f32, s: f32, ow: u32, oh: u32) -> Surface {
    composite_view_with(doc, vx, vy, s, ow, oh, &|_, _| None)
}

/// 뷰 재래스터용 벡터 아이템 — **월드 좌표** 기준. 호출자(wasm/CLI 셸)가 node.meta를
/// 해석해 제공하면, 뷰 합성이 저장된 래스터 표면 대신 이 벡터를 뷰 배율로 다시 굽는다
/// (Figma식: 확대해도 계단 없음, 축소 시도 깨끗한 AA). 문서/export 픽셀은 불변 —
/// 순수 표시 품질 경로다.
/// 뷰 아이템 채움 그라데이션 — 좌표는 도형 bbox 상대(0~1)라 스케일·줌 무관.
#[derive(Debug, Clone)]
pub struct ViewGrad {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub radial: bool,
    pub stops: Vec<(f32, [u8; 4])>,
}

#[derive(Debug, Clone)]
pub enum ViewItem {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rgba: [u8; 4],
        gradient: Option<ViewGrad>,
    },
    RoundedRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        rgba: [u8; 4],
        gradient: Option<ViewGrad>,
    },
    StrokeRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        width: f32,
        rgba: [u8; 4],
    },
    StrokeRoundedRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        width: f32,
        rgba: [u8; 4],
    },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        rgba: [u8; 4],
        gradient: Option<ViewGrad>,
    },
    StrokeEllipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        width: f32,
        rgba: [u8; 4],
    },
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        width: f32,
        rgba: [u8; 4],
    },
    Path {
        points: Vec<f32>,
        width: f32,
        rgba: [u8; 4],
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        rgba: [u8; 4],
        font: Option<String>,
    },
    Shadow {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        feather: f32,
        rgba: [u8; 4],
    },
    Polygon {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        sides: u32,
        rgba: [u8; 4],
        gradient: Option<ViewGrad>,
    },
    StrokePolygon {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        sides: u32,
        width: f32,
        rgba: [u8; 4],
    },
    Curve {
        points: Vec<f32>,
        width: f32,
        rgba: [u8; 4],
    },
    PolygonPath {
        points: Vec<f32>,
        rgba: [u8; 4],
        gradient: Option<ViewGrad>,
    },
    StrokePolygonPath {
        points: Vec<f32>,
        width: f32,
        rgba: [u8; 4],
    },
}

/// 점 목록 bbox (min_x, min_y, max_x, max_y).
fn view_points_bbox(points: &[f32]) -> Option<(f32, f32, f32, f32)> {
    if points.len() < 2 {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (points[0], points[1], points[0], points[1]);
    for p in points.chunks_exact(2) {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    Some((x0, y0, x1, y1))
}

/// 정다각형 꼭짓점(위 꼭짓점 시작) — dispatch::regular_polygon_points와 동일 정의.
fn view_polygon_points(cx: f32, cy: f32, rx: f32, ry: f32, sides: u32) -> Vec<f32> {
    let n = sides.clamp(3, 64);
    let mut pts = Vec::with_capacity(n as usize * 2);
    for k in 0..n {
        let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / n as f32;
        pts.push(cx + rx * a.cos());
        pts.push(cy + ry * a.sin());
    }
    pts
}

/// (노드, 스케일) → 스케일된-월드 좌표 재래스터 표면 + floor(min×s) 원점.
/// 호출자가 캐시를 소유한다(같은 줌에서 팬·재합성 시 재사용; 편집된 레이어만 다시 굽기).
/// None이면 래스터 표면 샘플로 폴백.
pub type VectorRender<'a> =
    &'a dyn Fn(&dcli_model::Node, f32) -> Option<(std::rc::Rc<Surface>, (i32, i32))>;

/// 노드의 **월드 좌표** 추가 경계 제공자(identity Paint 한정 의미) — meta 벡터 아이템이
/// 표면보다 넓을 수 있는 경우(예: set_props로 meta만 바꿔 그림자가 표면 밖으로 뻗는
/// 상태)를 컬링·그룹 tmp 크기 산정이 알게 한다. None = 표면 경계만 사용.
pub type AabbProvider<'a> = &'a dyn Fn(&dcli_model::Node) -> Option<(f32, f32, f32, f32)>;

/// 아이템 합집합 bbox(아이템과 같은 좌표계, 스트로크/feather 마진 포함) —
/// meta 기반 컬링·손상영역 박스 계산용.
pub fn view_items_bounds(items: &[ViewItem]) -> Option<(f32, f32, f32, f32)> {
    let mut u: Option<(f32, f32, f32, f32)> = None;
    for it in items {
        if let Some(b) = view_item_bounds(it) {
            u = Some(match u {
                Some(a) => (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)),
                None => b,
            });
        }
    }
    u
}

/// 월드 좌표 벡터 아이템들을 스케일 s로 굽는다(뷰 원점 무관 — 캐시 가능).
/// 반환 원점 = floor(min corner × s) − 1. 결과 픽셀 수가 max_px를 넘으면 None.
pub fn render_view_items(items: &[ViewItem], s: f32, max_px: u64) -> Option<(Surface, (i32, i32))> {
    let scaled: Vec<ViewItem> = items
        .iter()
        .map(|it| view_item_to_screen(it, 0.0, 0.0, s))
        .collect();
    let mut b: Option<(f32, f32, f32, f32)> = None;
    for it in &scaled {
        if let Some(bb) = view_item_bounds(it) {
            b = Some(match b {
                Some((a, c2, d, e)) => (a.min(bb.0), c2.min(bb.1), d.max(bb.2), e.max(bb.3)),
                None => bb,
            });
        }
    }
    let (x0f, y0f, x1f, y1f) = b?;
    let ox = (x0f.floor() as i32) - 1;
    let oy = (y0f.floor() as i32) - 1;
    let w = ((x1f.ceil() as i32) + 1 - ox).max(1) as u32;
    let h = ((y1f.ceil() as i32) + 1 - oy).max(1) as u32;
    if (w as u64) * (h as u64) > max_px {
        return None;
    }
    let mut tmp = Surface::new(w, h);
    for it in &scaled {
        draw_view_item(
            &mut tmp,
            &view_item_to_screen(it, ox as f32, oy as f32, 1.0),
        );
    }
    Some((tmp, (ox, oy)))
}

/// 뷰 합성(벡터 렌더 지원판). 자세한 계약은 composite_view 참조.
pub fn composite_view_with(
    doc: &Document,
    vx: f32,
    vy: f32,
    s: f32,
    ow: u32,
    oh: u32,
    vec_render: VectorRender,
) -> Surface {
    composite_view_impl(doc, vx, vy, s, ow, oh, None, &|_| None, vec_render, false)
}

/// **디스플레이 전용** 뷰 합성 — 감마 블렌드의 전달함수를 LUT(±1e-3)로 돌린다.
///
/// 반투명 레이어의 감마 블렌드는 픽셀당 powf ~9회로 화면 프레임의 지배 비용이다.
/// 이 경로는 표시 프레임에만 쓰고, export/PSD/골든/materialize는 정확 경로
/// (composite_view_with/composite/composite_region)를 유지한다 — 비트 계약 불변.
/// exclude = 화면에서만 제외할 노드(텍스트 인라인 편집) — 문서 clone 없이 스킵.
#[allow(clippy::too_many_arguments)]
pub fn composite_view_display(
    doc: &Document,
    vx: f32,
    vy: f32,
    s: f32,
    ow: u32,
    oh: u32,
    exclude: Option<dcli_model::NodeId>,
    aabb_of: AabbProvider,
    vec_render: VectorRender,
) -> Surface {
    composite_view_impl(doc, vx, vy, s, ow, oh, exclude, aabb_of, vec_render, true)
}

#[allow(clippy::too_many_arguments)]
fn composite_view_impl(
    doc: &Document,
    vx: f32,
    vy: f32,
    s: f32,
    ow: u32,
    oh: u32,
    exclude: Option<dcli_model::NodeId>,
    aabb_of: AabbProvider,
    vec_render: VectorRender,
    fast: bool,
) -> Surface {
    let mut acc = Surface::new(ow, oh);
    if s <= 0.0 {
        return acc;
    }
    for node in doc.iter_bottom_to_top() {
        composite_node_view(
            &mut acc,
            doc,
            node,
            (vx, vy, s),
            0,
            exclude,
            aabb_of,
            vec_render,
            fast,
        );
    }
    acc
}

/// 노드의 월드 좌표 AABB — 자기 트랜스폼 포함, **뷰 경로 의미론**(그룹 피벗 = 문서 중심).
///
/// 컬링·손상영역(damage) 계산용. 보수적(약간 큰) 박스는 무해하지만 작은 박스는 픽셀
/// 누락을 만든다 — Paint는 표면 4코너, Group은 자식 합집합 박스의 4코너를 변환한다.
/// 빈 그룹/표면 없음이면 None.
pub fn node_world_aabb(
    doc: &Document,
    node: &dcli_model::Node,
) -> Option<(f32, f32, f32, f32)> {
    node_world_aabb_inner(doc, node, &|_| None, 0)
}

/// node_world_aabb + 추가 경계 제공자: identity Paint 노드의 meta 벡터 아이템이 표면
/// 밖으로 뻗는 경우(그림자 등, set_props meta-only 흐름)를 박스에 합친다.
pub fn node_world_aabb_with(
    doc: &Document,
    node: &dcli_model::Node,
    extra: AabbProvider,
) -> Option<(f32, f32, f32, f32)> {
    node_world_aabb_inner(doc, node, extra, 0)
}

fn node_world_aabb_inner(
    doc: &Document,
    node: &dcli_model::Node,
    extra: AabbProvider,
    depth: u32,
) -> Option<(f32, f32, f32, f32)> {
    if depth > 32 {
        return None;
    }
    use dcli_model::NodeKind;
    let xf = |b: (f32, f32, f32, f32), c: (f32, f32)| -> (f32, f32, f32, f32) {
        let (ox, oy) = (node.offset.0 as f32, node.offset.1 as f32);
        if node.is_identity_transform() {
            return (b.0 + ox, b.1 + oy, b.2 + ox, b.3 + oy);
        }
        let (sin, cos) = node.rotation.to_radians().sin_cos();
        let (sx, sy) = node.scale;
        let map = |x: f32, y: f32| -> (f32, f32) {
            let wx = (x - c.0) * sx;
            let wy = (y - c.1) * sy;
            (cos * wx - sin * wy + c.0 + ox, sin * wx + cos * wy + c.1 + oy)
        };
        let ps = [map(b.0, b.1), map(b.2, b.1), map(b.0, b.3), map(b.2, b.3)];
        let minx = ps.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        let maxx = ps.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
        let miny = ps.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
        let maxy = ps.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
        (minx, miny, maxx, maxy)
    };
    match &node.kind {
        NodeKind::Paint { surface } => {
            let s = doc.pixels().get(*surface)?;
            let (sw, sh) = (s.width() as f32, s.height() as f32);
            // composite_layer_view와 동일: 피벗 = 표면 중심(src 좌표), offset은 변환 후.
            let surf_b = xf((0.0, 0.0, sw, sh), (sw * 0.5, sh * 0.5));
            // 벡터 재래스터는 identity 노드에서만 뛴다 — meta 경계도 그때만 의미 있다.
            if node.is_identity_transform() {
                if let Some(e) = extra(node) {
                    return Some((
                        surf_b.0.min(e.0),
                        surf_b.1.min(e.1),
                        surf_b.2.max(e.2),
                        surf_b.3.max(e.3),
                    ));
                }
            }
            Some(surf_b)
        }
        NodeKind::Group { children } => {
            let mut u: Option<(f32, f32, f32, f32)> = None;
            for cid in children {
                let Some(child) = doc.get(*cid) else { continue };
                if !child.visible || child.opacity <= 0.0 {
                    continue;
                }
                if let Some(b) = node_world_aabb_inner(doc, child, extra, depth + 1) {
                    u = Some(match u {
                        Some(a) => (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)),
                        None => b,
                    });
                }
            }
            // 그룹 트랜스폼 피벗 = 문서 중심(뷰 경로 cv와 동일 월드 고정 피벗).
            u.map(|b| xf(b, (doc.width as f32 * 0.5, doc.height as f32 * 0.5)))
        }
    }
}

/// 월드 아이템을 뷰 공간으로 변환(시프트 + 균일 스케일). 텍스트는 size·선폭까지 스케일.
fn view_item_to_screen(it: &ViewItem, vx: f32, vy: f32, s: f32) -> ViewItem {
    let tx = |x: f32| (x - vx) * s;
    let ty = |y: f32| (y - vy) * s;
    match it {
        ViewItem::Rect { x, y, w, h, rgba, gradient } => ViewItem::Rect {
            x: tx(*x),
            y: ty(*y),
            w: w * s,
            h: h * s,
            rgba: *rgba,
            gradient: gradient.clone(), // bbox 상대 좌표 — 스케일 불변.
        },
        ViewItem::RoundedRect {
            x,
            y,
            w,
            h,
            radius,
            rgba,
            gradient,
        } => ViewItem::RoundedRect {
            x: tx(*x),
            y: ty(*y),
            w: w * s,
            h: h * s,
            radius: radius * s,
            rgba: *rgba,
            gradient: gradient.clone(),
        },
        ViewItem::StrokeRect {
            x,
            y,
            w,
            h,
            width,
            rgba,
        } => ViewItem::StrokeRect {
            x: tx(*x),
            y: ty(*y),
            w: w * s,
            h: h * s,
            width: width * s,
            rgba: *rgba,
        },
        ViewItem::StrokeRoundedRect {
            x,
            y,
            w,
            h,
            radius,
            width,
            rgba,
        } => ViewItem::StrokeRoundedRect {
            x: tx(*x),
            y: ty(*y),
            w: w * s,
            h: h * s,
            radius: radius * s,
            width: width * s,
            rgba: *rgba,
        },
        ViewItem::Ellipse {
            cx,
            cy,
            rx,
            ry,
            rgba,
            gradient,
        } => ViewItem::Ellipse {
            cx: tx(*cx),
            cy: ty(*cy),
            rx: rx * s,
            ry: ry * s,
            rgba: *rgba,
            gradient: gradient.clone(),
        },
        ViewItem::StrokeEllipse {
            cx,
            cy,
            rx,
            ry,
            width,
            rgba,
        } => ViewItem::StrokeEllipse {
            cx: tx(*cx),
            cy: ty(*cy),
            rx: rx * s,
            ry: ry * s,
            width: width * s,
            rgba: *rgba,
        },
        ViewItem::Line {
            x0,
            y0,
            x1,
            y1,
            width,
            rgba,
        } => ViewItem::Line {
            x0: tx(*x0),
            y0: ty(*y0),
            x1: tx(*x1),
            y1: ty(*y1),
            width: width * s,
            rgba: *rgba,
        },
        ViewItem::Path {
            points,
            width,
            rgba,
        } => ViewItem::Path {
            points: points
                .chunks(2)
                .flat_map(|p| [tx(p[0]), ty(p[1])])
                .collect(),
            width: width * s,
            rgba: *rgba,
        },
        ViewItem::Text {
            x,
            y,
            text,
            size,
            rgba,
            font,
        } => ViewItem::Text {
            x: tx(*x),
            y: ty(*y),
            text: text.clone(),
            size: size * s,
            rgba: *rgba,
            font: font.clone(),
        },
        ViewItem::Shadow {
            x,
            y,
            w,
            h,
            radius,
            feather,
            rgba,
        } => ViewItem::Shadow {
            x: tx(*x),
            y: ty(*y),
            w: w * s,
            h: h * s,
            radius: radius * s,
            feather: feather * s,
            rgba: *rgba,
        },
        ViewItem::Polygon {
            cx,
            cy,
            rx,
            ry,
            sides,
            rgba,
            gradient,
        } => ViewItem::Polygon {
            cx: tx(*cx),
            cy: ty(*cy),
            rx: rx * s,
            ry: ry * s,
            sides: *sides,
            rgba: *rgba,
            gradient: gradient.clone(),
        },
        ViewItem::StrokePolygon {
            cx,
            cy,
            rx,
            ry,
            sides,
            width,
            rgba,
        } => ViewItem::StrokePolygon {
            cx: tx(*cx),
            cy: ty(*cy),
            rx: rx * s,
            ry: ry * s,
            sides: *sides,
            width: width * s,
            rgba: *rgba,
        },
        ViewItem::Curve {
            points,
            width,
            rgba,
        } => ViewItem::Curve {
            points: points
                .chunks(2)
                .flat_map(|p| [tx(p[0]), ty(p[1])])
                .collect(),
            width: width * s,
            rgba: *rgba,
        },
        ViewItem::PolygonPath {
            points,
            rgba,
            gradient,
        } => ViewItem::PolygonPath {
            points: points
                .chunks(2)
                .flat_map(|p| [tx(p[0]), ty(p[1])])
                .collect(),
            rgba: *rgba,
            gradient: gradient.clone(),
        },
        ViewItem::StrokePolygonPath {
            points,
            width,
            rgba,
        } => ViewItem::StrokePolygonPath {
            points: points
                .chunks(2)
                .flat_map(|p| [tx(p[0]), ty(p[1])])
                .collect(),
            width: width * s,
            rgba: *rgba,
        },
    }
}

/// 뷰(화면) 좌표 아이템의 AABB(여백 포함) — dispatch shape_bounds와 같은 마진 규약.
fn view_item_bounds(it: &ViewItem) -> Option<(f32, f32, f32, f32)> {
    match it {
        ViewItem::Rect { x, y, w, h, .. } | ViewItem::RoundedRect { x, y, w, h, .. } => {
            (*w > 0.0 && *h > 0.0).then_some((x - 1.0, y - 1.0, x + w + 1.0, y + h + 1.0))
        }
        ViewItem::StrokeRect {
            x, y, w, h, width, ..
        }
        | ViewItem::StrokeRoundedRect {
            x, y, w, h, width, ..
        } => {
            let m = width.max(1.0);
            (*w > 0.0 && *h > 0.0 && *width > 0.0).then_some((x - m, y - m, x + w + m, y + h + m))
        }
        ViewItem::Ellipse { cx, cy, rx, ry, .. } => (*rx > 0.0 && *ry > 0.0).then_some((
            cx - rx - 1.0,
            cy - ry - 1.0,
            cx + rx + 1.0,
            cy + ry + 1.0,
        )),
        ViewItem::StrokeEllipse {
            cx,
            cy,
            rx,
            ry,
            width,
            ..
        } => {
            let m = width.max(1.0);
            (*rx > 0.0 && *ry > 0.0 && *width > 0.0).then_some((
                cx - rx - m,
                cy - ry - m,
                cx + rx + m,
                cy + ry + m,
            ))
        }
        ViewItem::Line {
            x0,
            y0,
            x1,
            y1,
            width,
            ..
        } => {
            let m = width * 0.5 + 1.0;
            (*width > 0.0).then_some((
                x0.min(*x1) - m,
                y0.min(*y1) - m,
                x0.max(*x1) + m,
                y0.max(*y1) + m,
            ))
        }
        ViewItem::Path { points, width, .. } => {
            if points.len() < 4 || *width <= 0.0 {
                return None;
            }
            let m = width * 0.5 + 1.0;
            let (mut nx, mut ny, mut mx, mut my) = (
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            );
            for p in points.chunks(2) {
                nx = nx.min(p[0]);
                mx = mx.max(p[0]);
                ny = ny.min(p[1]);
                my = my.max(p[1]);
            }
            Some((nx - m, ny - m, mx + m, my + m))
        }
        ViewItem::Text {
            x, y, text, size, font, ..
        } => {
            if text.is_empty() || *size <= 0.0 {
                return None;
            }
            let (tw, th) = text::measure_text_font(text, *size, font.as_deref());
            let m = size.max(1.0) * 0.15 + 2.0;
            (tw > 0.0 && th > 0.0).then_some((x - m, y - m, x + tw + m, y + th + m))
        }
        ViewItem::Shadow {
            x,
            y,
            w,
            h,
            feather,
            ..
        } => {
            let m = feather.max(0.0) + 1.0;
            (*w > 0.0 && *h > 0.0).then_some((x - m, y - m, x + w + m, y + h + m))
        }
        ViewItem::Polygon { cx, cy, rx, ry, .. } => (*rx > 0.0 && *ry > 0.0).then_some((
            cx - rx - 1.0,
            cy - ry - 1.0,
            cx + rx + 1.0,
            cy + ry + 1.0,
        )),
        ViewItem::StrokePolygon {
            cx,
            cy,
            rx,
            ry,
            width,
            ..
        } => {
            let m = width.max(1.0);
            (*rx > 0.0 && *ry > 0.0 && *width > 0.0).then_some((
                cx - rx - m,
                cy - ry - m,
                cx + rx + m,
                cy + ry + m,
            ))
        }
        ViewItem::Curve { points, width, .. } => {
            if points.len() < 2 || *width <= 0.0 {
                return None;
            }
            // dispatch shape_bounds Curve와 동일: CR 오버슈트 마진 0.5×축별 최대 스텝.
            let (mut nx, mut ny, mut mx, mut my) = (points[0], points[1], points[0], points[1]);
            let (mut sx, mut sy) = (0.0f32, 0.0f32);
            for i in (2..points.len() - 1).step_by(2) {
                let (x, y) = (points[i], points[i + 1]);
                nx = nx.min(x);
                mx = mx.max(x);
                ny = ny.min(y);
                my = my.max(y);
                sx = sx.max((x - points[i - 2]).abs());
                sy = sy.max((y - points[i - 1]).abs());
            }
            let wx = width * 0.5 + 1.0 + 0.5 * sx;
            let wy = width * 0.5 + 1.0 + 0.5 * sy;
            Some((nx - wx, ny - wy, mx + wx, my + wy))
        }
        ViewItem::PolygonPath { points, .. } => {
            if points.len() < 6 {
                return None;
            }
            view_points_bbox(points).map(|(x0, y0, x1, y1)| (x0 - 1.0, y0 - 1.0, x1 + 1.0, y1 + 1.0))
        }
        ViewItem::StrokePolygonPath { points, width, .. } => {
            if points.len() < 6 || *width <= 0.0 {
                return None;
            }
            let m = width.max(1.0);
            view_points_bbox(points).map(|(x0, y0, x1, y1)| (x0 - m, y0 - m, x1 + m, y1 + m))
        }
    }
}

/// ViewGrad → 절대 px 색 콜백(도형 bbox 앵커).
///
/// stop 보간은 256-구간 LUT + 구간 내 lerp로 선계산한다 — per-px stop 탐색 제거.
/// gradient_color_at은 t에 대해 조각별 선형이라, 균일 격자 재표본 + lerp의 오차는
/// stop이 격자 사이에 있을 때만 생기고 8bit 표시에서 ≤1 LSB(다MP 채움의 핫패스).
fn view_grad_fn(g: &ViewGrad, bx: f32, by: f32, bw: f32, bh: f32) -> impl Fn(f32, f32) -> LinearPremul {
    const N: usize = 256;
    let lin = shapes::stops_to_linear(&g.stops);
    let lut: Vec<LinearPremul> = (0..=N)
        .map(|i| shapes::gradient_color_at(&lin, i as f32 / N as f32))
        .collect();
    let (ax, ay) = (bx + g.x0 * bw, by + g.y0 * bh);
    let (ex, ey) = (bx + g.x1 * bw, by + g.y1 * bh);
    let radial = g.radial;
    let dx = ex - ax;
    let dy = ey - ay;
    let len2 = (dx * dx + dy * dy).max(f32::EPSILON);
    let rad = len2.sqrt();
    move |px: f32, py: f32| {
        let t = if radial {
            (((px - ax).powi(2) + (py - ay).powi(2)).sqrt() / rad).clamp(0.0, 1.0)
        } else {
            (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0)
        };
        let f = t * N as f32;
        let i = (f as usize).min(N - 1);
        let fr = f - i as f32;
        let (c0, c1) = (lut[i], lut[i + 1]);
        LinearPremul {
            r: c0.r + (c1.r - c0.r) * fr,
            g: c0.g + (c1.g - c0.g) * fr,
            b: c0.b + (c1.b - c0.b) * fr,
            a: c0.a + (c1.a - c0.a) * fr,
        }
    }
}

fn draw_view_item(sfc: &mut Surface, it: &ViewItem) {
    match it {
        ViewItem::Rect { x, y, w, h, rgba, gradient } => match gradient {
            Some(g) => shapes::fill_rect_with(sfc, *x, *y, *w, *h, &view_grad_fn(g, *x, *y, *w, *h)),
            None => shapes::fill_rect(sfc, *x, *y, *w, *h, *rgba),
        },
        ViewItem::RoundedRect {
            x,
            y,
            w,
            h,
            radius,
            rgba,
            gradient,
        } => match gradient {
            Some(g) => shapes::fill_rounded_rect_with(
                sfc,
                *x,
                *y,
                *w,
                *h,
                *radius,
                &view_grad_fn(g, *x, *y, *w, *h),
            ),
            None => shapes::fill_rounded_rect(sfc, *x, *y, *w, *h, *radius, *rgba),
        },
        ViewItem::StrokeRect {
            x,
            y,
            w,
            h,
            width,
            rgba,
        } => shapes::stroke_rect(sfc, *x, *y, *w, *h, *width, *rgba),
        ViewItem::StrokeRoundedRect {
            x,
            y,
            w,
            h,
            radius,
            width,
            rgba,
        } => shapes::stroke_rounded_rect(sfc, *x, *y, *w, *h, *radius, *width, *rgba),
        ViewItem::Ellipse {
            cx,
            cy,
            rx,
            ry,
            rgba,
            gradient,
        } => match gradient {
            Some(g) => shapes::fill_ellipse_with(
                sfc,
                *cx,
                *cy,
                *rx,
                *ry,
                &view_grad_fn(g, cx - rx, cy - ry, rx * 2.0, ry * 2.0),
            ),
            None => shapes::fill_ellipse(sfc, *cx, *cy, *rx, *ry, *rgba),
        },
        ViewItem::StrokeEllipse {
            cx,
            cy,
            rx,
            ry,
            width,
            rgba,
        } => shapes::stroke_ellipse(sfc, *cx, *cy, *rx, *ry, *width, *rgba),
        ViewItem::Line {
            x0,
            y0,
            x1,
            y1,
            width,
            rgba,
        } => shapes::stroke_line(sfc, *x0, *y0, *x1, *y1, *width, *rgba),
        ViewItem::Path {
            points,
            width,
            rgba,
        } => {
            // dispatch draw_shape의 Path 규약과 동일: capsule 선분 연쇄.
            for seg in points.windows(4).step_by(2) {
                shapes::stroke_line(sfc, seg[0], seg[1], seg[2], seg[3], *width, *rgba);
            }
            if points.len() == 2 {
                shapes::stroke_line(
                    sfc, points[0], points[1], points[0], points[1], *width, *rgba,
                );
            }
        }
        ViewItem::Text {
            x,
            y,
            text,
            size,
            rgba,
            font,
        } => {
            text::draw_text_font(sfc, *x, *y, text, *size, *rgba, font.as_deref());
        }
        ViewItem::Shadow {
            x,
            y,
            w,
            h,
            radius,
            feather,
            rgba,
        } => shapes::fill_shadow(sfc, *x, *y, *w, *h, *radius, *feather, *rgba),
        ViewItem::Polygon {
            cx,
            cy,
            rx,
            ry,
            sides,
            rgba,
            gradient,
        } => {
            let pts = view_polygon_points(*cx, *cy, *rx, *ry, *sides);
            match gradient {
                Some(g) => shapes::fill_polygon_with(
                    sfc,
                    &pts,
                    &view_grad_fn(g, cx - rx, cy - ry, rx * 2.0, ry * 2.0),
                ),
                None => shapes::fill_polygon(sfc, &pts, *rgba),
            }
        }
        ViewItem::StrokePolygon {
            cx,
            cy,
            rx,
            ry,
            sides,
            width,
            rgba,
        } => {
            let pts = view_polygon_points(*cx, *cy, *rx, *ry, *sides);
            shapes::stroke_polygon(sfc, &pts, *width, *rgba);
        }
        ViewItem::Curve {
            points,
            width,
            rgba,
        } => shapes::stroke_curve(sfc, points, *width, *rgba),
        ViewItem::PolygonPath {
            points,
            rgba,
            gradient,
        } => match gradient {
            Some(g) => {
                let Some((x0, y0, x1, y1)) = view_points_bbox(points) else {
                    return;
                };
                shapes::fill_polygon_with(sfc, points, &view_grad_fn(g, x0, y0, x1 - x0, y1 - y0));
            }
            None => shapes::fill_polygon(sfc, points, *rgba),
        },
        ViewItem::StrokePolygonPath {
            points,
            width,
            rgba,
        } => shapes::stroke_polygon(sfc, points, *width, *rgba),
    }
}

/// 노드 하나를 뷰 공간 acc 위에 합성한다. view = (vx, vy, s).
#[allow(clippy::too_many_arguments)]
fn composite_node_view(
    acc: &mut Surface,
    doc: &Document,
    node: &dcli_model::Node,
    view: (f32, f32, f32),
    depth: u32,
    exclude: Option<dcli_model::NodeId>,
    aabb_of: AabbProvider,
    vec_render: VectorRender,
    fast: bool,
) {
    if !node.visible || node.opacity <= 0.0 || depth > 32 {
        return;
    }
    if exclude == Some(node.id) {
        return; // 화면 전용 제외(텍스트 인라인 편집) — 그룹이면 서브트리째 스킵.
    }
    let (vx, vy, s) = view;
    // 오프스크린 컬링: 뷰포트와 겹치지 않는 노드는 재래스터(벡터 캐시 미스)·tmp 할당·
    // 블렌드 비용을 아예 내지 않는다 — 장면 크기 독립성의 핵심. pad는 재래스터 ±1px
    // 마진 + AA 여유. (AABB를 못 구하는 노드는 보수적으로 통과.)
    if let Some(b) = node_world_aabb_with(doc, node, aabb_of) {
        let pad = 4.0;
        if (b.2 - vx) * s < -pad
            || (b.3 - vy) * s < -pad
            || (b.0 - vx) * s > acc.width() as f32 + pad
            || (b.1 - vy) * s > acc.height() as f32 + pad
        {
            return;
        }
    }
    use dcli_model::NodeKind;
    match &node.kind {
        NodeKind::Paint { surface } => {
            let Some(surface) = doc.pixels().get(*surface) else {
                return;
            };
            // 디바이스 1:1(s=1, 정수 원점) + identity 노드 = 정수 시프트 경로(비트 패리티).
            if node.is_identity_transform() && s == 1.0 && vx.fract() == 0.0 && vy.fract() == 0.0 {
                composite_layer(
                    acc,
                    surface.pixels(),
                    (surface.width(), surface.height()),
                    (node.offset.0 - vx as i32, node.offset.1 - vy as i32),
                    node.blend,
                    node.opacity,
                    doc.blend_space,
                    fast,
                );
                return;
            }
            // 벡터 경로: meta가 벡터(도형/브러시/텍스트)인 identity 노드는 저장 표면을
            // 업샘플하지 않고 뷰 배율 재래스터(캐시는 호출자 소유)를 정수 블렌드한다 —
            // 확대 계단현상 제거(Figma 동작). 회전·스케일 노드는 래스터 샘플로 폴백.
            if node.is_identity_transform() {
                if let Some((vsfc, vo)) = vec_render(node, s) {
                    composite_layer(
                        acc,
                        vsfc.pixels(),
                        (vsfc.width(), vsfc.height()),
                        (
                            vo.0 - (vx * s).round() as i32,
                            vo.1 - (vy * s).round() as i32,
                        ),
                        node.blend,
                        node.opacity,
                        doc.blend_space,
                        fast,
                    );
                    return;
                }
            }
            let c = (surface.width() as f32 * 0.5, surface.height() as f32 * 0.5);
            composite_layer_view(
                acc,
                surface.pixels(),
                (surface.width(), surface.height()),
                c,
                (node.offset.0 as f32, node.offset.1 as f32),
                node.scale,
                node.rotation,
                view,
                node.blend,
                node.opacity,
                doc.blend_space,
                fast,
            );
        }
        NodeKind::Group { children } => {
            if node.is_identity_transform() {
                // isolated group이지만 블렌드는 픽셀-로컬이므로, tmp를 자식 합집합
                // bbox∩뷰포트 크기로 잘라도 결과는 풀스크린 tmp와 비트 동일하다
                // (자식은 자기 범위 밖을 안 건드리고, 투명 src 블렌드는 no-op).
                // 풀스크린 tmp(뷰포트당 수십 MB) 할당·제로필·전면 블렌드 제거.
                let mut ub: Option<(f32, f32, f32, f32)> = None;
                for cid in children {
                    let Some(child) = doc.get(*cid) else { continue };
                    if !child.visible || child.opacity <= 0.0 {
                        continue;
                    }
                    if let Some(b) = node_world_aabb_with(doc, child, aabb_of) {
                        ub = Some(match ub {
                            Some(a) => (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)),
                            None => b,
                        });
                    }
                }
                let Some(ub) = ub else { return };
                // 그룹 offset은 뷰 px로 스냅해 블렌드 시점에 적용(자식 좌표계 tmp).
                let offdx = (node.offset.0 as f32 * s).round() as i32;
                let offdy = (node.offset.1 as f32 * s).round() as i32;
                let pad = 4i32;
                let bx0 = ((((ub.0 - vx) * s).floor() as i32) - pad).max(-offdx);
                let by0 = ((((ub.1 - vy) * s).floor() as i32) - pad).max(-offdy);
                let bx1 = ((((ub.2 - vx) * s).ceil() as i32) + pad).min(acc.width() as i32 - offdx);
                let by1 = ((((ub.3 - vy) * s).ceil() as i32) + pad).min(acc.height() as i32 - offdy);
                if bx1 <= bx0 || by1 <= by0 {
                    return;
                }
                let mut tmp = Surface::new((bx1 - bx0) as u32, (by1 - by0) as u32);
                // 자식 뷰 원점을 정수 뷰 px만큼 이동 — (vx·s).round()+bx0 불변(시프트 불변성)
                // 이라 벡터 블릿·s=1 비트 경로 모두 풀프레임과 일치한다.
                let sub = (vx + bx0 as f32 / s, vy + by0 as f32 / s, s);
                for cid in children {
                    if let Some(child) = doc.get(*cid) {
                        composite_node_view(&mut tmp, doc, child, sub, depth + 1, exclude, aabb_of, vec_render, fast);
                    }
                }
                composite_layer(
                    acc,
                    tmp.pixels(),
                    (tmp.width(), tmp.height()),
                    (bx0 + offdx, by0 + offdy),
                    node.blend,
                    node.opacity,
                    doc.blend_space,
                    fast,
                );
            } else {
                // 트랜스폼 그룹(드묾): 자식을 같은 뷰 공간 풀사이즈 tmp에 합성 후 변환.
                let mut tmp = Surface::new(acc.width(), acc.height());
                for cid in children {
                    if let Some(child) = doc.get(*cid) {
                        composite_node_view(&mut tmp, doc, child, view, depth + 1, exclude, aabb_of, vec_render, fast);
                    }
                }
                // 그룹 트랜스폼을 뷰 공간으로 옮긴다: 월드 피벗 C(문서 중심)는 뷰에서
                // Cv = (C − v)·s, offset은 F·s. (doc-res 경로의 region-center 피벗
                // 의존성은 따르지 않는다 — 전체 문서 export와 동일한 월드 고정 피벗.)
                let cv = (
                    (doc.width as f32 * 0.5 - vx) * s,
                    (doc.height as f32 * 0.5 - vy) * s,
                );
                composite_layer_view(
                    acc,
                    // tmp를 빌리는 동안 acc에 쓰기 위해 복사 없이 분리: tmp는 별도 표면.
                    tmp.pixels(),
                    (tmp.width(), tmp.height()),
                    cv,
                    (node.offset.0 as f32 * s, node.offset.1 as f32 * s),
                    node.scale,
                    node.rotation,
                    (0.0, 0.0, 1.0),
                    node.blend,
                    node.opacity,
                    doc.blend_space,
                    fast,
                );
            }
        }
    }
}

/// 일반화 뷰 샘플 합성 — 명시적 피벗 c(src 좌표), f32 offset, 뷰 변환.
///
/// forward: out(p) = (R(θ)·(S⊙(p − c)) + c + off − v) · s
/// inverse(out px X 중심): u = (X+0.5)/s + v;  p = S⁻¹⊙R(−θ)·(u − c − off) + c.
/// 샘플은 composite_layer_transformed와 동일한 bilinear(픽셀중심 격자, 밖은 투명).
#[allow(clippy::too_many_arguments)]
fn composite_layer_view(
    acc: &mut Surface,
    src: &[LinearPremul],
    src_dim: (u32, u32),
    c: (f32, f32),
    off: (f32, f32),
    scale: (f32, f32),
    rotation_deg: f32,
    view: (f32, f32, f32),
    blend: BlendMode,
    opacity: f32,
    space: BlendSpace,
    fast: bool,
) {
    let (dw, dh) = (acc.width() as i32, acc.height() as i32);
    let (sw, sh) = (src_dim.0 as f32, src_dim.1 as f32);
    let (swi, shi) = (src_dim.0 as i32, src_dim.1 as i32);
    let (sx, sy) = scale;
    let (vx, vy, vs) = view;
    if sx.abs() < 1e-4 || sy.abs() < 1e-4 || vs <= 0.0 {
        return;
    }
    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    let (cx, cy) = c;
    let (ox, oy) = off;

    // 출력 AABB = src 4코너의 forward 변환(뷰 px) + 1px AA 마진.
    let fwd = |px: f32, py: f32| -> (f32, f32) {
        let wx = (px - cx) * sx;
        let wy = (py - cy) * sy;
        (
            (cos * wx - sin * wy + cx + ox - vx) * vs,
            (sin * wx + cos * wy + cy + oy - vy) * vs,
        )
    };
    let corners = [fwd(0.0, 0.0), fwd(sw, 0.0), fwd(0.0, sh), fwd(sw, sh)];
    let minx = corners.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
    let maxx = corners
        .iter()
        .map(|p| p.0)
        .fold(f32::NEG_INFINITY, f32::max);
    let miny = corners.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
    let maxy = corners
        .iter()
        .map(|p| p.1)
        .fold(f32::NEG_INFINITY, f32::max);
    let x0 = ((minx.floor() as i32) - 1).clamp(0, dw);
    let x1 = ((maxx.ceil() as i32) + 1).clamp(0, dw);
    let y0 = ((miny.floor() as i32) - 1).clamp(0, dh);
    let y1 = ((maxy.ceil() as i32) + 1).clamp(0, dh);

    let zero = LinearPremul {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    let tap = |ix: i32, iy: i32| -> LinearPremul {
        if ix < 0 || iy < 0 || ix >= swi || iy >= shi {
            zero
        } else {
            src[(iy * swi + ix) as usize]
        }
    };
    let lerp = |a: LinearPremul, b: LinearPremul, t: f32| LinearPremul {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    };

    let inv_s = 1.0 / vs;
    // 출력 1px이 원본 여러 px을 덮는 축소(minification)에서는 단일 bilinear가 원본을
    // 건너뛰어 계단·모아레가 생긴다 → 2×2 슈퍼샘플(서브픽셀 4점 평균)로 완화.
    // (벡터 meta 레이어는 이 함수에 오지 않고 타깃 해상도 재래스터를 탄다.)
    let eff = vs * sx.abs().min(sy.abs());
    let supersample = eff < 0.75;
    let sample_at = |out_x: f32, out_y: f32| -> LinearPremul {
        let ux = out_x * inv_s + vx - ox - cx;
        let uy = out_y * inv_s + vy - oy - cy;
        let rx = cos * ux + sin * uy;
        let ry = -sin * ux + cos * uy;
        let psx = rx / sx + cx;
        let psy = ry / sy + cy;
        let fx = psx - 0.5;
        let fy = psy - 0.5;
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
    let dst = acc.pixels_mut();
    for y in y0..y1 {
        for x in x0..x1 {
            let sp = if supersample {
                let s00 = sample_at(x as f32 + 0.25, y as f32 + 0.25);
                let s10 = sample_at(x as f32 + 0.75, y as f32 + 0.25);
                let s01 = sample_at(x as f32 + 0.25, y as f32 + 0.75);
                let s11 = sample_at(x as f32 + 0.75, y as f32 + 0.75);
                LinearPremul {
                    r: (s00.r + s10.r + s01.r + s11.r) * 0.25,
                    g: (s00.g + s10.g + s01.g + s11.g) * 0.25,
                    b: (s00.b + s10.b + s01.b + s11.b) * 0.25,
                    a: (s00.a + s10.a + s01.a + s11.a) * 0.25,
                }
            } else {
                sample_at(x as f32 + 0.5, y as f32 + 0.5)
            };
            if sp.a <= 0.0 {
                continue;
            }
            let di = (y * dw + x) as usize;
            dst[di] = blend_pixel(dst[di], sp, blend, opacity, space, fast);
        }
    }
}

/// 노드 하나(페인트 또는 그룹)를 acc 위에 합성한다. region = 출력 원점의 월드 좌표.
fn composite_node(
    acc: &mut Surface,
    doc: &Document,
    node: &dcli_model::Node,
    region: (i32, i32),
    depth: u32,
) {
    if !node.visible || node.opacity <= 0.0 || depth > 32 {
        return;
    }
    use dcli_model::NodeKind;
    match &node.kind {
        NodeKind::Paint { surface } => {
            let Some(surface) = doc.pixels().get(*surface) else {
                return; // 표면 없음(예: JSON만 로드) — 건너뜀.
            };
            if node.is_identity_transform() {
                // 정수 시프트 경로: 영역 원점만큼 offset을 당기면 끝(비트동일 유지).
                composite_layer(
                    acc,
                    surface.pixels(),
                    (surface.width(), surface.height()),
                    (node.offset.0 - region.0, node.offset.1 - region.1),
                    node.blend,
                    node.opacity,
                    doc.blend_space,
                    false,
                );
            } else {
                composite_layer_transformed(
                    acc,
                    surface.pixels(),
                    (surface.width(), surface.height()),
                    node.offset,
                    node.scale,
                    node.rotation,
                    node.blend,
                    node.opacity,
                    doc.blend_space,
                    region,
                );
            }
        }
        NodeKind::Group { children } => {
            // isolated group: 자식들을 영역과 동일 원점/크기의 임시 표면에 먼저 합성한 뒤,
            // 그 결과를 그룹 props/transform을 가진 레이어처럼 acc에 얹는다.
            let mut tmp = Surface::new(acc.width(), acc.height());
            for cid in children {
                if let Some(child) = doc.get(*cid) {
                    composite_node(&mut tmp, doc, child, region, depth + 1);
                }
            }
            // tmp는 이미 영역-로컬 좌표 → 그룹 트랜스폼은 영역 기준 (0,0)으로 적용.
            if node.is_identity_transform() {
                composite_layer(
                    acc,
                    tmp.pixels(),
                    (tmp.width(), tmp.height()),
                    node.offset,
                    node.blend,
                    node.opacity,
                    doc.blend_space,
                    false,
                );
            } else {
                composite_layer_transformed(
                    acc,
                    tmp.pixels(),
                    (tmp.width(), tmp.height()),
                    node.offset,
                    node.scale,
                    node.rotation,
                    node.blend,
                    node.opacity,
                    doc.blend_space,
                    (0, 0),
                );
            }
        }
    }
}

/// 한 레이어를 누적 표면 위에 블렌드한다. `offset`(dx,dy)만큼 시프트해 그린다.
///
/// dst 픽셀 (x,y)는 src 픽셀 (x-dx, y-dy)에서 읽는다. offset=(0,0)이면 1:1로 기존과 동일.
/// 경계 밖(src 범위 이탈)은 투명이므로 블렌드 기여 없음(건너뜀).
#[allow(clippy::too_many_arguments)]
fn composite_layer(
    acc: &mut Surface,
    src: &[LinearPremul],
    src_dim: (u32, u32),
    offset: (i32, i32),
    blend: BlendMode,
    opacity: f32,
    space: BlendSpace,
    fast: bool,
) {
    let (dw, dh) = (acc.width() as i32, acc.height() as i32);
    let (sw, sh) = (src_dim.0 as i32, src_dim.1 as i32);
    let (dx, dy) = offset;

    // 빠른 경로: offset 0 + 동일 크기면 기존 1:1 zip(비트동일·최속).
    if dx == 0 && dy == 0 && sw == dw && sh == dh {
        let dst = acc.pixels_mut();
        debug_assert_eq!(dst.len(), src.len());
        for (d, s) in dst.iter_mut().zip(src.iter()) {
            *d = blend_pixel(*d, *s, blend, opacity, space, fast);
        }
        return;
    }

    // 일반 경로: dst가 src와 겹치는 영역만 순회(경계 밖은 투명 = noop).
    let dst = acc.pixels_mut();
    let x0 = dx.max(0);
    let y0 = dy.max(0);
    let x1 = (dx + sw).min(dw);
    let y1 = (dy + sh).min(dh);
    for y in y0..y1 {
        for x in x0..x1 {
            let s = src[((y - dy) * sw + (x - dx)) as usize];
            let di = (y * dw + x) as usize;
            dst[di] = blend_pixel(dst[di], s, blend, opacity, space, fast);
        }
    }
}

/// 트랜스폼(스케일·회전) 레이어 합성 — 역변환 + bilinear 리샘플(비파괴).
///
/// 변환 정의(GPU wgsl과 1:1 동일 수학이어야 parity 게이트 통과):
///   T(p_src) = R(θ)·(S ⊙ (p_src − c)) + c + offset,  c = 표면 중심, θ = 시계방향(도).
/// dst 픽셀 중심을 역변환해 src 픽셀중심 격자에서 bilinear 샘플한다(premul 공간 —
/// premul 값의 선형 보간은 색 번짐 없이 정확). 격자 밖 탭은 투명 → 가장자리 1px AA.
#[allow(clippy::too_many_arguments)]
fn composite_layer_transformed(
    acc: &mut Surface,
    src: &[LinearPremul],
    src_dim: (u32, u32),
    offset: (i32, i32),
    scale: (f32, f32),
    rotation_deg: f32,
    blend: BlendMode,
    opacity: f32,
    space: BlendSpace,
    // region: 출력 (0,0)의 월드 좌표 — 영역 합성(composite_region) 지원. 전체 문서면 (0,0).
    region: (i32, i32),
) {
    let (dw, dh) = (acc.width() as i32, acc.height() as i32);
    let (sw, sh) = (src_dim.0 as f32, src_dim.1 as f32);
    let (swi, shi) = (src_dim.0 as i32, src_dim.1 as i32);
    let (sx, sy) = scale;
    if sx.abs() < 1e-4 || sy.abs() < 1e-4 {
        return; // 퇴화 스케일 → 보이는 것 없음.
    }
    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    let (cx, cy) = (sw * 0.5, sh * 0.5);
    let (ox, oy) = (offset.0 as f32, offset.1 as f32);
    let (rgx, rgy) = (region.0 as f32, region.1 as f32);

    // dst 바운딩 박스 = src 4코너의 forward 변환(월드) − 영역 원점, AABB(+1px AA 마진).
    let fwd = |px: f32, py: f32| -> (f32, f32) {
        let vx = (px - cx) * sx;
        let vy = (py - cy) * sy;
        (
            cos * vx - sin * vy + cx + ox - rgx,
            sin * vx + cos * vy + cy + oy - rgy,
        )
    };
    let corners = [fwd(0.0, 0.0), fwd(sw, 0.0), fwd(0.0, sh), fwd(sw, sh)];
    let minx = corners.iter().map(|c| c.0).fold(f32::INFINITY, f32::min);
    let maxx = corners
        .iter()
        .map(|c| c.0)
        .fold(f32::NEG_INFINITY, f32::max);
    let miny = corners.iter().map(|c| c.1).fold(f32::INFINITY, f32::min);
    let maxy = corners
        .iter()
        .map(|c| c.1)
        .fold(f32::NEG_INFINITY, f32::max);
    let x0 = ((minx.floor() as i32) - 1).clamp(0, dw);
    let x1 = ((maxx.ceil() as i32) + 1).clamp(0, dw);
    let y0 = ((miny.floor() as i32) - 1).clamp(0, dh);
    let y1 = ((maxy.ceil() as i32) + 1).clamp(0, dh);

    let zero = LinearPremul {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    let tap = |ix: i32, iy: i32| -> LinearPremul {
        if ix < 0 || iy < 0 || ix >= swi || iy >= shi {
            zero
        } else {
            src[(iy * swi + ix) as usize]
        }
    };
    let lerp = |a: LinearPremul, b: LinearPremul, t: f32| LinearPremul {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    };

    let dst = acc.pixels_mut();
    for y in y0..y1 {
        for x in x0..x1 {
            // 역변환: q = p_dst(월드 = 영역픽셀 + 영역원점) − off − c; r = R(−θ)q; p_src = r/S + c.
            let qx = x as f32 + rgx + 0.5 - ox - cx;
            let qy = y as f32 + rgy + 0.5 - oy - cy;
            let rx = cos * qx + sin * qy;
            let ry = -sin * qx + cos * qy;
            let psx = rx / sx + cx;
            let psy = ry / sy + cy;
            // bilinear: 픽셀중심(i+0.5) 격자.
            let fx = psx - 0.5;
            let fy = psy - 0.5;
            let ix = fx.floor() as i32;
            let iy = fy.floor() as i32;
            let tx = fx - ix as f32;
            let ty = fy - iy as f32;
            let sp = lerp(
                lerp(tap(ix, iy), tap(ix + 1, iy), tx),
                lerp(tap(ix, iy + 1), tap(ix + 1, iy + 1), tx),
                ty,
            );
            if sp.a <= 0.0 {
                continue; // 완전 투명 — 블렌드 기여 없음.
            }
            let di = (y * dw + x) as usize;
            // 트랜스폼 합성은 export/materialize 경로 전용 — 항상 정확 블렌드.
            dst[di] = blend_pixel(dst[di], sp, blend, opacity, space, false);
        }
    }
}

/// 단일 픽셀 블렌드. 색공간 분기의 핵심.
///
/// `dst`/`src` 모두 linear-premul. `opacity`는 src에 곱해지는 레이어 불투명도.
#[inline]
fn blend_pixel(
    dst: LinearPremul,
    src: LinearPremul,
    blend: BlendMode,
    opacity: f32,
    space: BlendSpace,
    fast: bool,
) -> LinearPremul {
    // 레이어 opacity를 src alpha와 premul 색에 동시 적용(premul 불변식 유지).
    let src = LinearPremul {
        r: src.r * opacity,
        g: src.g * opacity,
        b: src.b * opacity,
        a: src.a * opacity,
    };

    // ── fast path (성능 지배 경로) ──
    // 감마 블렌드는 픽셀당 OETF/EOTF powf ~9회라, 이 두 지름길 없이는 레이어 수십 개
    // 문서의 영역 합성이 수백 ms로 떨어진다. 둘 다 수학적으로 정확한 항등이라
    // (왕복 인코딩 오차를 오히려 제거) 의미론 변화가 아니다.
    // 1) 완전 투명 src는 기여 없음 — dst 그대로(영역 밖 투명을 건너뛰는 기존 계약과 동일).
    if src.a <= 0.0 {
        return dst;
    }
    // 2) Normal × 완전 불투명 src는 dst를 정확히 src로 대체.
    if matches!(blend, BlendMode::Normal) && src.a >= 1.0 {
        return src;
    }

    match space {
        BlendSpace::Linear => blend_in_linear(dst, src, blend),
        // fast = 디스플레이 프레임 전용(LUT ±1e-3). 정확 경로는 위 항등 fast path와
        // 합쳐져 비트 계약(골든/parity/export)을 유지한다.
        BlendSpace::Gamma if fast => blend_in_gamma_fast(dst, src, blend),
        BlendSpace::Gamma => blend_in_gamma(dst, src, blend),
    }
}

/// linear-light 공간에서 블렌드 (32bit/native 경로). premul over.
#[inline]
fn blend_in_linear(dst: LinearPremul, src: LinearPremul, blend: BlendMode) -> LinearPremul {
    // separable 블렌드 함수를 linear premul 성분에 적용한 뒤 over.
    let (br, bg, bb) = blend_rgb_premul(
        (dst.r, dst.g, dst.b),
        (src.r, src.g, src.b),
        dst.a,
        src.a,
        blend,
    );
    over(dst, src, (br, bg, bb))
}

/// 감마(sRGB 인코딩) 공간에서 블렌드 (8/16bit Photoshop 경로).
///
/// 내부 저장은 linear-premul이므로:
/// 1) un-premultiply → straight linear
/// 2) per-component OETF로 감마 인코딩 (Photoshop이 실제로 블렌드하는 값)
/// 3) 감마 값에 블렌드 수학 적용
/// 4) 결과를 다시 선형화(EOTF) → premultiply
#[inline]
fn blend_in_gamma(dst: LinearPremul, src: LinearPremul, blend: BlendMode) -> LinearPremul {
    blend_in_gamma_with(dst, src, blend, srgb_eotf, srgb_oetf)
}

/// **디스플레이 전용** 감마 블렌드 — 전달함수를 LUT(±1e-3)로. 비트 계약 경로 금지.
#[inline]
fn blend_in_gamma_fast(dst: LinearPremul, src: LinearPremul, blend: BlendMode) -> LinearPremul {
    blend_in_gamma_with(dst, src, blend, srgb_eotf_fast, srgb_oetf_fast)
}

/// 감마 블렌드 본체 — 전달함수만 주입받는다(정확/LUT 경로가 블렌드 수학을 공유,
/// 제네릭 단형화라 정확 경로의 인라인·비트 결과는 그대로).
#[inline]
fn blend_in_gamma_with(
    dst: LinearPremul,
    src: LinearPremul,
    blend: BlendMode,
    eotf: impl Fn(f32) -> f32,
    oetf: impl Fn(f32) -> f32,
) -> LinearPremul {
    let dg = to_straight_gamma_with(dst, &oetf);
    let sg = to_straight_gamma_with(src, &oetf);

    // 감마 공간 straight 성분에 블렌드 수학.
    let blended_gamma = match blend {
        BlendMode::Normal => sg.rgb,
        BlendMode::Multiply => (
            dg.rgb.0 * sg.rgb.0,
            dg.rgb.1 * sg.rgb.1,
            dg.rgb.2 * sg.rgb.2,
        ),
        BlendMode::Screen => (
            screen(dg.rgb.0, sg.rgb.0),
            screen(dg.rgb.1, sg.rgb.1),
            screen(dg.rgb.2, sg.rgb.2),
        ),
        BlendMode::Darken => (
            dg.rgb.0.min(sg.rgb.0),
            dg.rgb.1.min(sg.rgb.1),
            dg.rgb.2.min(sg.rgb.2),
        ),
        BlendMode::Lighten => (
            dg.rgb.0.max(sg.rgb.0),
            dg.rgb.1.max(sg.rgb.1),
            dg.rgb.2.max(sg.rgb.2),
        ),
        BlendMode::Overlay => (
            overlay(dg.rgb.0, sg.rgb.0),
            overlay(dg.rgb.1, sg.rgb.1),
            overlay(dg.rgb.2, sg.rgb.2),
        ),
        BlendMode::Difference => (
            (dg.rgb.0 - sg.rgb.0).abs(),
            (dg.rgb.1 - sg.rgb.1).abs(),
            (dg.rgb.2 - sg.rgb.2).abs(),
        ),
    };

    // 블렌드된 감마 색을 다시 linear straight로.
    let blended_lin = (
        eotf(blended_gamma.0),
        eotf(blended_gamma.1),
        eotf(blended_gamma.2),
    );

    // alpha는 색공간과 무관하게 선형으로 합성. over compositing의 알파/색 결합:
    // Photoshop의 감마 블렌드도 결과색은 src_a 기준으로 dst 위에 얹는다.
    // 여기서는 blended_lin을 "src의 새 색"으로 보고 linear premul over.
    let sa = src.a;
    let da = dst.a;
    let out_a = sa + da * (1.0 - sa);
    // blended_lin(straight) * sa = premul 기여.
    let or = blended_lin.0 * sa + dst.r * (1.0 - sa);
    let og = blended_lin.1 * sa + dst.g * (1.0 - sa);
    let ob = blended_lin.2 * sa + dst.b * (1.0 - sa);

    LinearPremul {
        r: or,
        g: og,
        b: ob,
        a: out_a,
    }
}

struct StraightGamma {
    /// straight(비-premul), 감마 인코딩된 sRGB 성분.
    rgb: (f32, f32, f32),
}

/// linear-premul → straight 감마 인코딩. alpha==0 가드. 전달함수 주입형.
#[inline]
fn to_straight_gamma_with(p: LinearPremul, oetf: &impl Fn(f32) -> f32) -> StraightGamma {
    if p.a <= 0.0 {
        return StraightGamma {
            rgb: (0.0, 0.0, 0.0),
        };
    }
    let inv = 1.0 / p.a;
    StraightGamma {
        rgb: (
            oetf((p.r * inv).clamp(0.0, 1.0)),
            oetf((p.g * inv).clamp(0.0, 1.0)),
            oetf((p.b * inv).clamp(0.0, 1.0)),
        ),
    }
}

/// linear premul 성분에 직접 블렌드 함수 적용 (linear 경로용).
///
/// premul 값에 multiply/screen을 적용하는 표준 정의를 따른다.
#[inline]
fn blend_rgb_premul(
    dst: (f32, f32, f32),
    src: (f32, f32, f32),
    _da: f32,
    _sa: f32,
    blend: BlendMode,
) -> (f32, f32, f32) {
    match blend {
        BlendMode::Normal => src,
        BlendMode::Multiply => (dst.0 * src.0, dst.1 * src.1, dst.2 * src.2),
        BlendMode::Screen => (
            screen(dst.0, src.0),
            screen(dst.1, src.1),
            screen(dst.2, src.2),
        ),
        // 신규 4종도 기존 Multiply/Screen과 같은 관행으로 premul 성분에 직접
        // 동일 공식을 적용한다 (linear 경로의 단순화된 정의 — GPU wgsl과 1:1).
        BlendMode::Darken => (dst.0.min(src.0), dst.1.min(src.1), dst.2.min(src.2)),
        BlendMode::Lighten => (dst.0.max(src.0), dst.1.max(src.1), dst.2.max(src.2)),
        BlendMode::Overlay => (
            overlay(dst.0, src.0),
            overlay(dst.1, src.1),
            overlay(dst.2, src.2),
        ),
        BlendMode::Difference => (
            (dst.0 - src.0).abs(),
            (dst.1 - src.1).abs(),
            (dst.2 - src.2).abs(),
        ),
    }
}

/// 블렌드된 src 색(premul)을 dst 위에 over.
#[inline]
fn over(dst: LinearPremul, src: LinearPremul, blended_src_rgb: (f32, f32, f32)) -> LinearPremul {
    let sa = src.a;
    let out_a = sa + dst.a * (1.0 - sa);
    LinearPremul {
        r: blended_src_rgb.0 + dst.r * (1.0 - sa),
        g: blended_src_rgb.1 + dst.g * (1.0 - sa),
        b: blended_src_rgb.2 + dst.b * (1.0 - sa),
        a: out_a,
    }
}

#[inline]
fn screen(a: f32, b: f32) -> f32 {
    1.0 - (1.0 - a) * (1.0 - b)
}

/// Overlay: dst가 어두우면 multiply 계열(2ds), 밝으면 screen 계열(1−2(1−d)(1−s)).
#[inline]
fn overlay(d: f32, s: f32) -> f32 {
    if d <= 0.5 {
        2.0 * d * s
    } else {
        1.0 - 2.0 * (1.0 - d) * (1.0 - s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcli_color::BitDepth;
    use dcli_model::{NodeProps, Op};
    use dcli_tile::Surface;

    fn solid(w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Surface {
        Surface::filled(w, h, LinearPremul::from_srgb8_straight(r, g, b, a))
    }

    /// 표면을 등록하고 페인트 레이어를 맨 위에 추가, blend를 설정한다.
    fn add(doc: &mut Document, name: &str, s: Surface, blend: BlendMode) {
        let sid = doc.add_surface(s);
        Op::AddPaintLayer {
            name: name.into(),
            surface: sid,
            index: None,
            forced_id: None,
        }
        .apply(doc)
        .unwrap();
        let id = *doc.order().last().unwrap();
        Op::SetProps {
            id,
            props: NodeProps {
                name: name.into(),
                visible: true,
                opacity: 1.0,
                blend,
                offset: (0, 0),
                scale: (1.0, 1.0),
                rotation: 0.0,
                meta: None,
            },
        }
        .apply(doc)
        .unwrap();
    }

    #[test]
    fn opaque_normal_shows_top() {
        let mut doc = Document::new(2, 2, BitDepth::U8);
        add(
            &mut doc,
            "bg",
            solid(2, 2, 255, 0, 0, 255),
            BlendMode::Normal,
        );
        add(
            &mut doc,
            "top",
            solid(2, 2, 0, 0, 255, 255),
            BlendMode::Normal,
        );
        let out = composite(&doc).to_srgb8_rgba();
        assert_eq!(&out[0..4], &[0, 0, 255, 255]);
    }

    #[test]
    fn view_s1_integer_matches_region() {
        // s=1 + 정수 원점 뷰 = composite_region과 비트 동일(정수 시프트 경로 핀).
        let mut doc = Document::new(64, 48, BitDepth::U8);
        add(
            &mut doc,
            "bg",
            solid(64, 48, 240, 235, 226, 255),
            BlendMode::Normal,
        );
        add(
            &mut doc,
            "dot",
            solid(8, 6, 20, 90, 200, 255),
            BlendMode::Normal,
        );
        let id = *doc.order().last().unwrap();
        doc.get_mut(id).unwrap().offset = (10, 12);
        let region = composite_region(&doc, 4, 6, 40, 30).to_srgb8_rgba();
        let view = composite_view(&doc, 4.0, 6.0, 1.0, 40, 30).to_srgb8_rgba();
        assert_eq!(region, view, "s=1 뷰가 region 합성과 비트동일이어야");
    }

    #[test]
    fn view_scale_2x_doubles_content() {
        // 2배 뷰: (10..18)의 8px 사각형이 출력에서 16px 폭으로.
        let mut doc = Document::new(64, 48, BitDepth::U8);
        add(
            &mut doc,
            "dot",
            solid(8, 6, 255, 0, 0, 255),
            BlendMode::Normal,
        );
        let id = *doc.order().last().unwrap();
        doc.get_mut(id).unwrap().offset = (10, 12);
        let out = composite_view(&doc, 0.0, 0.0, 2.0, 128, 96);
        let px = |x: u32, y: u32| out.pixels()[(y * 128 + x) as usize].a;
        assert!(px(21, 25) > 0.9, "사각형 내부(2배 좌표)는 불투명");
        assert!(px(37, 29) < 0.1, "사각형 밖은 투명");
    }

    #[test]
    fn view_vector_render_rerasters() {
        // 벡터 렌더 제공 시 표면 대신 뷰 배율 재래스터 — 4배 확대에서 가장자리가 선명해야
        // (업샘플이면 경계 알파가 0.25~0.75 사이로 뭉개진다. 재래스터는 1px AA 이내).
        let mut doc = Document::new(64, 48, BitDepth::U8);
        // 표면은 더미지만 **아이템 범위를 덮어야** 한다(프로덕션 불변식: 표면 =
        // meta 아이템의 materialize 결과 — 오프스크린 컬링이 이 불변식에 기댄다).
        add(&mut doc, "rect", solid(22, 22, 0, 0, 0, 0), BlendMode::Normal);
        let id = *doc.order().last().unwrap();
        doc.get_mut(id).unwrap().offset = (9, 9);
        let items = vec![ViewItem::Rect {
            x: 10.0,
            y: 10.0,
            w: 20.0,
            h: 20.0,
            rgba: [255, 0, 0, 255],
            gradient: None,
        }];
        let render: VectorRender = &|_n, s| {
            render_view_items(&items, s, 16_000_000).map(|(sf, o)| (std::rc::Rc::new(sf), o))
        };
        let out = composite_view_with(&doc, 8.0, 8.0, 4.0, 128, 128, render);
        let px = |x: u32, y: u32| out.pixels()[(y * 128 + x) as usize].a;
        // 사각형 좌변: 월드 x=10 → 뷰 x=(10−8)*4=8. 내부(20,40)는 완전 불투명, 경계 한 픽셀만 AA.
        assert!(px(20, 40) > 0.99, "내부 완전 불투명(재래스터)");
        assert!(px(6, 40) < 0.01, "경계 밖 투명");
    }

    #[test]
    fn gamma_vs_linear_differ_for_multiply() {
        // 같은 입력이라도 감마/리니어 경로 결과가 달라야 한다(핵심 가설).
        let mut g = Document::new(1, 1, BitDepth::U8); // Gamma
        add(
            &mut g,
            "bg",
            solid(1, 1, 128, 128, 128, 255),
            BlendMode::Normal,
        );
        add(
            &mut g,
            "t",
            solid(1, 1, 128, 128, 128, 255),
            BlendMode::Multiply,
        );

        let mut l = Document::new(1, 1, BitDepth::F32); // Linear
        add(
            &mut l,
            "bg",
            solid(1, 1, 128, 128, 128, 255),
            BlendMode::Normal,
        );
        add(
            &mut l,
            "t",
            solid(1, 1, 128, 128, 128, 255),
            BlendMode::Multiply,
        );

        let og = composite(&g).to_srgb8_rgba();
        let ol = composite(&l).to_srgb8_rgba();
        assert_ne!(og, ol, "감마/리니어 합성이 동일하면 분기가 무의미");
    }

    #[test]
    fn gamma_multiply_matches_photoshop_arithmetic() {
        // Photoshop 감마 multiply: 50%회색 x 50%회색 = 25%회색(감마값 기준).
        let mut g = Document::new(1, 1, BitDepth::U8);
        add(
            &mut g,
            "bg",
            solid(1, 1, 128, 128, 128, 255),
            BlendMode::Normal,
        );
        add(
            &mut g,
            "t",
            solid(1, 1, 128, 128, 128, 255),
            BlendMode::Multiply,
        );
        let out = composite(&g).to_srgb8_rgba();
        let expected = dcli_color::quantize_u8(0.501_960_8_f32 * 0.501_960_8);
        assert!(
            (out[0] as i32 - expected as i32).abs() <= 1,
            "got {} expected {}",
            out[0],
            expected
        );
    }

    #[test]
    fn gamma_darken_lighten_pick_min_max() {
        // 감마 공간: 50%회색(128) 위 25%회색(64).
        // darken = min(감마값) → 64, lighten = max(감마값) → 128.
        let mut d = Document::new(1, 1, BitDepth::U8);
        add(
            &mut d,
            "bg",
            solid(1, 1, 128, 128, 128, 255),
            BlendMode::Normal,
        );
        add(&mut d, "t", solid(1, 1, 64, 64, 64, 255), BlendMode::Darken);
        let out = composite(&d).to_srgb8_rgba();
        assert!((out[0] as i32 - 64).abs() <= 1, "darken got {}", out[0]);

        let mut l = Document::new(1, 1, BitDepth::U8);
        add(
            &mut l,
            "bg",
            solid(1, 1, 128, 128, 128, 255),
            BlendMode::Normal,
        );
        add(
            &mut l,
            "t",
            solid(1, 1, 64, 64, 64, 255),
            BlendMode::Lighten,
        );
        let out = composite(&l).to_srgb8_rgba();
        assert!((out[0] as i32 - 128).abs() <= 1, "lighten got {}", out[0]);
    }

    #[test]
    fn gamma_overlay_both_branches() {
        // 감마 공간 overlay 양쪽 분기 검증 (d = dst 감마값 기준 분기).
        let d128 = 128.0_f32 / 255.0; // ≈0.50196 > 0.5 → screen 분기
        let d64 = 64.0_f32 / 255.0; //  ≈0.25098 ≤ 0.5 → multiply 분기

        // dst=50%회색(128), src=25%회색(64): 1 − 2(1−d)(1−s).
        let mut a = Document::new(1, 1, BitDepth::U8);
        add(
            &mut a,
            "bg",
            solid(1, 1, 128, 128, 128, 255),
            BlendMode::Normal,
        );
        add(
            &mut a,
            "t",
            solid(1, 1, 64, 64, 64, 255),
            BlendMode::Overlay,
        );
        let out = composite(&a).to_srgb8_rgba();
        let expected = dcli_color::quantize_u8(1.0 - 2.0 * (1.0 - d128) * (1.0 - d64));
        assert!(
            (out[0] as i32 - expected as i32).abs() <= 1,
            "overlay(screen 분기) got {} expected {}",
            out[0],
            expected
        );

        // dst=25%회색(64), src=50%회색(128): 2ds.
        let mut b = Document::new(1, 1, BitDepth::U8);
        add(
            &mut b,
            "bg",
            solid(1, 1, 64, 64, 64, 255),
            BlendMode::Normal,
        );
        add(
            &mut b,
            "t",
            solid(1, 1, 128, 128, 128, 255),
            BlendMode::Overlay,
        );
        let out = composite(&b).to_srgb8_rgba();
        let expected = dcli_color::quantize_u8(2.0 * d64 * d128);
        assert!(
            (out[0] as i32 - expected as i32).abs() <= 1,
            "overlay(multiply 분기) got {} expected {}",
            out[0],
            expected
        );
    }

    #[test]
    fn gamma_difference_is_abs_delta() {
        // 감마 공간: |128/255 − 64/255| = 64/255 → 64.
        let mut d = Document::new(1, 1, BitDepth::U8);
        add(
            &mut d,
            "bg",
            solid(1, 1, 128, 128, 128, 255),
            BlendMode::Normal,
        );
        add(
            &mut d,
            "t",
            solid(1, 1, 64, 64, 64, 255),
            BlendMode::Difference,
        );
        let out = composite(&d).to_srgb8_rgba();
        assert!((out[0] as i32 - 64).abs() <= 1, "difference got {}", out[0]);
    }

    #[test]
    fn offset_shifts_layer() {
        // 4x4 문서, 좌상단 1픽셀짜리 빨강을 (2,1)만큼 이동 → (2,1)에 나타나고 (0,0)은 투명.
        let mut doc = Document::new(4, 4, BitDepth::U8);
        // 표면 자체는 (0,0)에 빨강, 나머지 투명.
        let mut s = Surface::new(4, 4);
        s.pixels_mut()[0] = LinearPremul::from_srgb8_straight(255, 0, 0, 255);
        let sid = doc.add_surface(s);
        Op::AddPaintLayer {
            name: "dot".into(),
            surface: sid,
            index: None,
            forced_id: None,
        }
        .apply(&mut doc)
        .unwrap();
        let id = *doc.order().last().unwrap();
        // offset (2,1) 적용.
        doc.get_mut(id).unwrap().offset = (2, 1);

        let out = composite(&doc).to_srgb8_rgba();
        let at = |x: usize, y: usize| {
            let i = (y * 4 + x) * 4;
            &out[i..i + 4]
        };
        assert_eq!(at(0, 0), &[0, 0, 0, 0], "원위치는 비어야");
        assert_eq!(at(2, 1), &[255, 0, 0, 255], "(2,1)로 이동");
    }

    #[test]
    fn offset_zero_bit_identical_to_unshifted() {
        // offset (0,0)은 기존 1:1 경로와 비트동일해야(빠른 경로 회귀 핀).
        let mut a = Document::new(3, 3, BitDepth::U8);
        add(&mut a, "x", solid(3, 3, 12, 34, 56, 200), BlendMode::Normal);
        let mut b = Document::new(3, 3, BitDepth::U8);
        add(&mut b, "x", solid(3, 3, 12, 34, 56, 200), BlendMode::Normal);
        b.get_mut(*b.order().last().unwrap()).unwrap().offset = (0, 0);
        assert_eq!(composite(&a).to_srgb8_rgba(), composite(&b).to_srgb8_rgba());
    }

    #[test]
    fn scale_2x_enlarges_dot() {
        // 중심 (8,8) 근처 2x2 빨강을 2배 스케일 → 중심 기준 4x4 영역으로 확대.
        let mut doc = Document::new(16, 16, BitDepth::U8);
        let mut s = Surface::new(16, 16);
        for y in 7..9 {
            for x in 7..9 {
                s.pixels_mut()[y * 16 + x] = LinearPremul::from_srgb8_straight(255, 0, 0, 255);
            }
        }
        let sid = doc.add_surface(s);
        Op::AddPaintLayer {
            name: "dot".into(),
            surface: sid,
            index: None,
            forced_id: None,
        }
        .apply(&mut doc)
        .unwrap();
        let id = *doc.order().last().unwrap();
        doc.get_mut(id).unwrap().scale = (2.0, 2.0);

        let out = composite(&doc).to_srgb8_rgba();
        let a = |x: usize, y: usize| out[(y * 16 + x) * 4 + 3];
        // 원본 2x2(7..9)가 중심(8,8) 기준 2배 → 내부(7,7)/(8,8)는 완전 불투명,
        // 가장자리(6,6)는 bilinear 부분 커버(>100), 멀리(2,2)는 투명.
        assert_eq!(a(7, 7), 255, "확대 내부 (7,7)");
        assert_eq!(a(8, 8), 255, "확대 내부 (8,8)");
        assert!(a(6, 6) > 100, "확대 가장자리 AA (6,6): {}", a(6, 6));
        assert_eq!(a(2, 2), 0, "확대 밖 투명");
    }

    #[test]
    fn rotation_90_moves_pixel() {
        // (12,8)의 점을 중심(8,8) 기준 90° 시계 회전 → (8,12)로 이동해야.
        let mut doc = Document::new(16, 16, BitDepth::U8);
        let mut s = Surface::new(16, 16);
        s.pixels_mut()[8 * 16 + 12] = LinearPremul::from_srgb8_straight(0, 255, 0, 255);
        let sid = doc.add_surface(s);
        Op::AddPaintLayer {
            name: "p".into(),
            surface: sid,
            index: None,
            forced_id: None,
        }
        .apply(&mut doc)
        .unwrap();
        let id = *doc.order().last().unwrap();
        doc.get_mut(id).unwrap().rotation = 90.0;

        let out = composite(&doc).to_srgb8_rgba();
        let a = |x: usize, y: usize| out[(y * 16 + x) * 4 + 3];
        // src 픽셀 (12,8) 중심 (12.5,8.5), 중심 (8,8) 기준 90° 시계 →
        // (−vy, vx) = (−0.5, 4.5) → dst 중심 (7.5, 12.5) = 픽셀 (7,12) 정중앙.
        assert_eq!(a(7, 12), 255, "90° 회전 후 (7,12): {}", a(7, 12));
        assert_eq!(a(12, 8), 0, "원위치는 비어야");
    }

    #[test]
    fn identity_transform_bit_identical() {
        // scale=(1,1), rotation=0이면 기존 경로와 비트동일(fast path 핀).
        let mut a = Document::new(5, 5, BitDepth::U8);
        add(
            &mut a,
            "x",
            solid(5, 5, 90, 120, 150, 230),
            BlendMode::Multiply,
        );
        let mut b = Document::new(5, 5, BitDepth::U8);
        add(
            &mut b,
            "x",
            solid(5, 5, 90, 120, 150, 230),
            BlendMode::Multiply,
        );
        let id = *b.order().last().unwrap();
        b.get_mut(id).unwrap().scale = (1.0, 1.0);
        b.get_mut(id).unwrap().rotation = 0.0;
        assert_eq!(composite(&a).to_srgb8_rgba(), composite(&b).to_srgb8_rgba());
    }

    #[test]
    fn transparent_layer_is_noop() {
        let mut doc = Document::new(1, 1, BitDepth::U8);
        add(
            &mut doc,
            "bg",
            solid(1, 1, 10, 20, 30, 255),
            BlendMode::Normal,
        );
        add(
            &mut doc,
            "clear",
            solid(1, 1, 200, 200, 200, 0),
            BlendMode::Normal,
        );
        let out = composite(&doc).to_srgb8_rgba();
        assert_eq!(&out[0..4], &[10, 20, 30, 255]);
    }
}

#[cfg(test)]
mod region_group_tests {
    use super::*;
    use dcli_color::BitDepth;
    use dcli_model::{Document, NodeKind, Op};
    use dcli_tile::Surface;

    fn red_dot_doc() -> Document {
        // (10,10)에 빨강 1픽셀.
        let mut doc = Document::new(32, 32, BitDepth::U8);
        let mut s = Surface::new(32, 32);
        s.pixels_mut()[10 * 32 + 10] = LinearPremul::from_srgb8_straight(255, 0, 0, 255);
        let sid = doc.add_surface(s);
        Op::AddPaintLayer {
            name: "dot".into(),
            surface: sid,
            index: None,
            forced_id: None,
        }
        .apply(&mut doc)
        .unwrap();
        doc
    }

    #[test]
    fn region_window_shifts_content() {
        // 영역 (8,8,8,8)로 자르면 빨강은 출력 (2,2)에 와야 한다.
        let doc = red_dot_doc();
        let out = composite_region(&doc, 8, 8, 8, 8).to_srgb8_rgba();
        let a = |x: usize, y: usize| out[(y * 8 + x) * 4 + 3];
        assert_eq!(a(2, 2), 255, "영역-로컬 (2,2)에 빨강");
        assert_eq!(a(0, 0), 0);
        // 음수 원점 영역(무한 작업영역): (-10,-10) 원점이면 빨강은 (20,20).
        let out2 = composite_region(&doc, -10, -10, 32, 32).to_srgb8_rgba();
        let a2 = |x: usize, y: usize| out2[(y * 32 + x) * 4 + 3];
        assert_eq!(a2(20, 20), 255, "음수 원점 보정");
    }

    #[test]
    fn region_full_doc_equals_composite() {
        let doc = red_dot_doc();
        assert_eq!(
            composite(&doc).to_srgb8_rgba(),
            composite_region(&doc, 0, 0, 32, 32).to_srgb8_rgba(),
            "전체 영역 = composite 비트동일"
        );
    }

    #[test]
    fn group_opacity_applies_to_composited_children() {
        // 두 불투명 레이어를 그룹으로 묶고 그룹 opacity 0.5 → 결과는 한 번만 반투명
        // (isolated group: 자식 먼저 합성 후 그룹 속성 적용).
        let mut doc = Document::new(4, 4, BitDepth::U8);
        for _ in 0..2 {
            let s = Surface::filled(4, 4, LinearPremul::from_srgb8_straight(255, 0, 0, 255));
            let sid = doc.add_surface(s);
            Op::AddPaintLayer {
                name: "r".into(),
                surface: sid,
                index: None,
                forced_id: None,
            }
            .apply(&mut doc)
            .unwrap();
        }
        let (a, b) = (doc.order()[0], doc.order()[1]);
        Op::GroupLayers {
            ids: vec![a, b],
            name: "g".into(),
            forced_id: None,
        }
        .apply(&mut doc)
        .unwrap();
        let gid = doc.order()[0];
        doc.get_mut(gid).unwrap().opacity = 0.5;

        let out = composite(&doc).to_srgb8_rgba();
        // 흰 배경 없음(투명 위) → premul 알파 절반.
        assert!(
            (out[3] as i32 - 128).abs() <= 2,
            "그룹 opacity 1회 적용: a={}",
            out[3]
        );
        assert!(out[0] > 245, "straight 빨강 유지: r={}", out[0]);
    }

    #[test]
    fn group_offset_moves_children_together() {
        let mut doc = red_dot_doc();
        let id = doc.order()[0];
        Op::GroupLayers {
            ids: vec![id],
            name: "g".into(),
            forced_id: None,
        }
        .apply(&mut doc)
        .unwrap();
        let gid = doc.order()[0];
        doc.get_mut(gid).unwrap().offset = (5, 3);
        let out = composite(&doc).to_srgb8_rgba();
        let a = |x: usize, y: usize| out[(y * 32 + x) * 4 + 3];
        assert_eq!(a(15, 13), 255, "그룹 offset로 자식 함께 이동");
        assert_eq!(a(10, 10), 0);
    }
}
