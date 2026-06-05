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
        composite_layer(&mut acc, surface.pixels(), node.blend, node.opacity, doc.blend_space);
    }
    acc
}

/// 한 레이어를 누적 표면 위에 블렌드한다.
fn composite_layer(
    acc: &mut Surface,
    src: &[LinearPremul],
    blend: BlendMode,
    opacity: f32,
    space: BlendSpace,
) {
    let dst = acc.pixels_mut();
    debug_assert_eq!(dst.len(), src.len());
    for (d, s) in dst.iter_mut().zip(src.iter()) {
        *d = blend_pixel(*d, *s, blend, opacity, space);
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
            props: NodeProps { name: name.into(), visible: true, opacity: 1.0, blend },
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
    fn transparent_layer_is_noop() {
        let mut doc = Document::new(1, 1, BitDepth::U8);
        add(&mut doc, "bg", solid(1, 1, 10, 20, 30, 255), BlendMode::Normal);
        add(&mut doc, "clear", solid(1, 1, 200, 200, 200, 0), BlendMode::Normal);
        let out = composite(&doc).to_srgb8_rgba();
        assert_eq!(&out[0..4], &[10, 20, 30, 255]);
    }
}
