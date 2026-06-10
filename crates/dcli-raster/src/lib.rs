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

use dcli_color::{srgb_eotf, srgb_oetf, BlendSpace, LinearPremul};
use dcli_model::{BlendMode, Document};
use dcli_tile::Surface;

/// 문서를 한 장의 표면으로 합성한다 (CPU 정본).
///
/// 노드를 bottom-to-top으로 순회하며 누적한다. 페인트 노드의 표면은 PixelStore에서
/// SurfaceId로 조회한다(픽셀 인라인 금지 규칙). 결과는 linear-premul 표면.
pub fn composite(doc: &Document) -> Surface {
    let mut acc = Surface::new(doc.width, doc.height);
    for node in doc.iter_bottom_to_top() {
        if !node.visible || node.opacity <= 0.0 {
            continue;
        }
        let Some(sid) = node.surface_id() else {
            // 그룹 등 픽셀 없는 노드는 Phase 1에서 합성 기여 없음(후속 Phase에서 확장).
            continue;
        };
        let Some(surface) = doc.pixels().get(sid) else {
            // 표면이 스토어에 없으면(예: JSON만 로드) 건너뛴다.
            continue;
        };
        if node.is_identity_transform() {
            // 스케일 1·회전 0 → 기존 정수 시프트 경로(비트동일·최속).
            composite_layer(
                &mut acc,
                surface.pixels(),
                (surface.width(), surface.height()),
                node.offset,
                node.blend,
                node.opacity,
                doc.blend_space,
            );
        } else {
            composite_layer_transformed(
                &mut acc,
                surface.pixels(),
                (surface.width(), surface.height()),
                node.offset,
                node.scale,
                node.rotation,
                node.blend,
                node.opacity,
                doc.blend_space,
            );
        }
    }
    acc
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
) {
    let (dw, dh) = (acc.width() as i32, acc.height() as i32);
    let (sw, sh) = (src_dim.0 as i32, src_dim.1 as i32);
    let (dx, dy) = offset;

    // 빠른 경로: offset 0 + 동일 크기면 기존 1:1 zip(비트동일·최속).
    if dx == 0 && dy == 0 && sw == dw && sh == dh {
        let dst = acc.pixels_mut();
        debug_assert_eq!(dst.len(), src.len());
        for (d, s) in dst.iter_mut().zip(src.iter()) {
            *d = blend_pixel(*d, *s, blend, opacity, space);
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
            dst[di] = blend_pixel(dst[di], s, blend, opacity, space);
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

    // dst 바운딩 박스 = src 4코너의 forward 변환 AABB(+1px AA 마진).
    let fwd = |px: f32, py: f32| -> (f32, f32) {
        let vx = (px - cx) * sx;
        let vy = (py - cy) * sy;
        (cos * vx - sin * vy + cx + ox, sin * vx + cos * vy + cy + oy)
    };
    let corners = [fwd(0.0, 0.0), fwd(sw, 0.0), fwd(0.0, sh), fwd(sw, sh)];
    let minx = corners.iter().map(|c| c.0).fold(f32::INFINITY, f32::min);
    let maxx = corners.iter().map(|c| c.0).fold(f32::NEG_INFINITY, f32::max);
    let miny = corners.iter().map(|c| c.1).fold(f32::INFINITY, f32::min);
    let maxy = corners.iter().map(|c| c.1).fold(f32::NEG_INFINITY, f32::max);
    let x0 = ((minx.floor() as i32) - 1).clamp(0, dw);
    let x1 = ((maxx.ceil() as i32) + 1).clamp(0, dw);
    let y0 = ((miny.floor() as i32) - 1).clamp(0, dh);
    let y1 = ((maxy.ceil() as i32) + 1).clamp(0, dh);

    let zero = LinearPremul { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
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
            // 역변환: q = p_dst − off − c; r = R(−θ)q; p_src = r/S + c.
            let qx = x as f32 + 0.5 - ox - cx;
            let qy = y as f32 + 0.5 - oy - cy;
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
            dst[di] = blend_pixel(dst[di], sp, blend, opacity, space);
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
) -> LinearPremul {
    // 레이어 opacity를 src alpha와 premul 색에 동시 적용(premul 불변식 유지).
    let src = LinearPremul {
        r: src.r * opacity,
        g: src.g * opacity,
        b: src.b * opacity,
        a: src.a * opacity,
    };

    match space {
        BlendSpace::Linear => blend_in_linear(dst, src, blend),
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
    let dg = to_straight_gamma(dst);
    let sg = to_straight_gamma(src);

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
    };

    // 블렌드된 감마 색을 다시 linear straight로.
    let blended_lin = (
        srgb_eotf(blended_gamma.0),
        srgb_eotf(blended_gamma.1),
        srgb_eotf(blended_gamma.2),
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

    LinearPremul { r: or, g: og, b: ob, a: out_a }
}

struct StraightGamma {
    /// straight(비-premul), 감마 인코딩된 sRGB 성분.
    rgb: (f32, f32, f32),
}

/// linear-premul → straight 감마 인코딩. alpha==0 가드.
#[inline]
fn to_straight_gamma(p: LinearPremul) -> StraightGamma {
    if p.a <= 0.0 {
        return StraightGamma { rgb: (0.0, 0.0, 0.0) };
    }
    let inv = 1.0 / p.a;
    StraightGamma {
        rgb: (
            srgb_oetf((p.r * inv).clamp(0.0, 1.0)),
            srgb_oetf((p.g * inv).clamp(0.0, 1.0)),
            srgb_oetf((p.b * inv).clamp(0.0, 1.0)),
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
        Op::AddPaintLayer { name: name.into(), surface: sid, index: None, forced_id: None }
            .apply(doc)
            .unwrap();
        let id = *doc.order().last().unwrap();
        Op::SetProps {
            id,
            props: NodeProps { name: name.into(), visible: true, opacity: 1.0, blend, offset: (0, 0), scale: (1.0, 1.0), rotation: 0.0, meta: None },
        }
        .apply(doc)
        .unwrap();
    }

    #[test]
    fn opaque_normal_shows_top() {
        let mut doc = Document::new(2, 2, BitDepth::U8);
        add(&mut doc, "bg", solid(2, 2, 255, 0, 0, 255), BlendMode::Normal);
        add(&mut doc, "top", solid(2, 2, 0, 0, 255, 255), BlendMode::Normal);
        let out = composite(&doc).to_srgb8_rgba();
        assert_eq!(&out[0..4], &[0, 0, 255, 255]);
    }

    #[test]
    fn gamma_vs_linear_differ_for_multiply() {
        // 같은 입력이라도 감마/리니어 경로 결과가 달라야 한다(핵심 가설).
        let mut g = Document::new(1, 1, BitDepth::U8); // Gamma
        add(&mut g, "bg", solid(1, 1, 128, 128, 128, 255), BlendMode::Normal);
        add(&mut g, "t", solid(1, 1, 128, 128, 128, 255), BlendMode::Multiply);

        let mut l = Document::new(1, 1, BitDepth::F32); // Linear
        add(&mut l, "bg", solid(1, 1, 128, 128, 128, 255), BlendMode::Normal);
        add(&mut l, "t", solid(1, 1, 128, 128, 128, 255), BlendMode::Multiply);

        let og = composite(&g).to_srgb8_rgba();
        let ol = composite(&l).to_srgb8_rgba();
        assert_ne!(og, ol, "감마/리니어 합성이 동일하면 분기가 무의미");
    }

    #[test]
    fn gamma_multiply_matches_photoshop_arithmetic() {
        // Photoshop 감마 multiply: 50%회색 x 50%회색 = 25%회색(감마값 기준).
        let mut g = Document::new(1, 1, BitDepth::U8);
        add(&mut g, "bg", solid(1, 1, 128, 128, 128, 255), BlendMode::Normal);
        add(&mut g, "t", solid(1, 1, 128, 128, 128, 255), BlendMode::Multiply);
        let out = composite(&g).to_srgb8_rgba();
        let expected = dcli_color::quantize_u8(0.501_960_8_f32 * 0.501_960_8);
        assert!((out[0] as i32 - expected as i32).abs() <= 1, "got {} expected {}", out[0], expected);
    }

    #[test]
    fn offset_shifts_layer() {
        // 4x4 문서, 좌상단 1픽셀짜리 빨강을 (2,1)만큼 이동 → (2,1)에 나타나고 (0,0)은 투명.
        let mut doc = Document::new(4, 4, BitDepth::U8);
        // 표면 자체는 (0,0)에 빨강, 나머지 투명.
        let mut s = Surface::new(4, 4);
        s.pixels_mut()[0] = LinearPremul::from_srgb8_straight(255, 0, 0, 255);
        let sid = doc.add_surface(s);
        Op::AddPaintLayer { name: "dot".into(), surface: sid, index: None, forced_id: None }
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
        Op::AddPaintLayer { name: "dot".into(), surface: sid, index: None, forced_id: None }
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
        Op::AddPaintLayer { name: "p".into(), surface: sid, index: None, forced_id: None }
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
        add(&mut a, "x", solid(5, 5, 90, 120, 150, 230), BlendMode::Multiply);
        let mut b = Document::new(5, 5, BitDepth::U8);
        add(&mut b, "x", solid(5, 5, 90, 120, 150, 230), BlendMode::Multiply);
        let id = *b.order().last().unwrap();
        b.get_mut(id).unwrap().scale = (1.0, 1.0);
        b.get_mut(id).unwrap().rotation = 0.0;
        assert_eq!(composite(&a).to_srgb8_rgba(), composite(&b).to_srgb8_rgba());
    }

    #[test]
    fn transparent_layer_is_noop() {
        let mut doc = Document::new(1, 1, BitDepth::U8);
        add(&mut doc, "bg", solid(1, 1, 10, 20, 30, 255), BlendMode::Normal);
        add(&mut doc, "clear", solid(1, 1, 200, 200, 200, 0), BlendMode::Normal);
        let out = composite(&doc).to_srgb8_rgba();
        assert_eq!(&out[0..4], &[10, 20, 30, 255]);
    }
}
