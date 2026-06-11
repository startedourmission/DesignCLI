//! 색 변환 contract의 단일 출처.
//!
//! DesignCLI 전체에서 **가장 위험한 설계 결정**(gamma-vs-linear-landmine)이 여기 산다.
//! 이 crate의 함수들이 틀리면 합성·블렌드·export 전부가 Photoshop과 어긋난다.
//!
//! 규칙 (순서 재배열 금지):
//! - IMPORT: dequantize(8b=/255, **16b=/32768** NOT /65535) → (mode→RGB → ICC) →
//!   선형화(진짜 sRGB piecewise EOTF, **naive pow 2.2 금지**) → premultiply
//! - EXPORT: 역순 (un-premultiply, alpha==0 가드 → de-linearize(진짜 OETF) → quantize)
//! - **premultiply는 반드시 선형화 후.**
//!
//! 합성 색공간은 **비트깊이로 분기**한다:
//! - 8/16bit → 감마(sRGB 인코딩) 공간에서 블렌딩 (Photoshop 기본, `BlendSpace::Gamma`)
//! - 32bit/HDR → linear-light 공간에서 블렌딩 (`BlendSpace::Linear`)
//!
//! 이 둘은 진짜 다른 코드 경로다. 같은 경로일 수 없다.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// 문서/노드의 합성 색공간. 비트깊이로 분기된다(gamma-vs-linear-landmine).
///
/// `Gamma` = 감마 인코딩된 sRGB 값에 직접 블렌드 수학을 적용 (Photoshop 8/16bit 기본).
/// `Linear` = 선형화 후 블렌드 (물리 정확, Photoshop 32bit 및 "native-linear" 경로).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlendSpace {
    /// Photoshop 일치 경로 (8/16bit 기본). 감마 공간에서 블렌딩.
    Gamma,
    /// 물리 정확 경로 (32bit/HDR). linear-light에서 블렌딩.
    Linear,
}

impl BlendSpace {
    /// 문서 비트깊이에 따른 기본 합성 색공간.
    ///
    /// 8/16bit는 Photoshop 충실도를 위해 감마 공간을 기본으로 가정한다
    /// (PSD에 모드 플래그가 없어 파일에서 의도 감지 불가 → 기본값=감마).
    /// 32bit는 linear.
    pub const fn for_bit_depth(depth: BitDepth) -> Self {
        match depth {
            BitDepth::U8 | BitDepth::U16 => BlendSpace::Gamma,
            BitDepth::F32 => BlendSpace::Linear,
        }
    }
}

/// 문서/채널 비트깊이. dequantize 분모가 여기서 갈린다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BitDepth {
    /// 8-bit unsigned, dequantize = /255.
    U8,
    /// 16-bit unsigned, **dequantize = /32768** (Photoshop 규약; /65535 아님).
    U16,
    /// 32-bit float, 이미 [0,1] 정규화 가정.
    F32,
}

// ---------------------------------------------------------------------------
// sRGB 전달함수 (진짜 piecewise, naive pow 2.2 절대 금지)
// ---------------------------------------------------------------------------

/// sRGB EOTF: 감마 인코딩된 sRGB 성분([0,1]) → linear-light([0,1]).
///
/// IEC 61966-2-1 piecewise. naive `pow(x, 2.2)`는 저역에서 수 % 어긋나며
/// 가장자리 어두워짐·색조 이동을 일으키므로 금지.
#[inline]
pub fn srgb_eotf(c: f32) -> f32 {
    if c <= 0.040_448_237 {
        // 선형 구간. (임계값 = 0.04045 근방, f32 정밀도로 명시)
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// sRGB OETF (역 EOTF): linear-light([0,1]) → 감마 인코딩된 sRGB([0,1]).
#[inline]
pub fn srgb_oetf(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

// ---------------------------------------------------------------------------
// dequantize / quantize (비트깊이 분기)
// ---------------------------------------------------------------------------

/// 정수 샘플 → 정규화 [0,1]. 16bit는 /32768.
#[inline]
pub fn dequantize(sample: u32, depth: BitDepth) -> f32 {
    match depth {
        BitDepth::U8 => sample as f32 / 255.0,
        // Photoshop 16bit 규약: 0..=32768. /65535면 흰색 근처 ~2배 어두워지는 실제 버그.
        BitDepth::U16 => sample as f32 / 32768.0,
        BitDepth::F32 => f32::from_bits(sample),
    }
}

/// 정규화 [0,1] → 8bit 정수(round, clamp).
#[inline]
pub fn quantize_u8(c: f32) -> u8 {
    (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// 정규화 [0,1] → 16bit 정수(/32768 규약 역, round, clamp to 32768).
#[inline]
pub fn quantize_u16(c: f32) -> u16 {
    (c.clamp(0.0, 1.0) * 32768.0 + 0.5).min(32768.0) as u16
}

// ---------------------------------------------------------------------------
// RGBA: 직선(straight) alpha vs premultiplied
// ---------------------------------------------------------------------------

/// linear-light, premultiplied alpha RGBA. 합성기의 내부 작업 표현.
///
/// 불변식: rgb는 alpha로 premultiply된 상태이며 색공간은 항상 linear-light다.
/// (블렌드를 감마 공간에서 하더라도 *저장*은 linear premul로 통일 — gamma-vs-linear-landmine.)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearPremul {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl LinearPremul {
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// 8bit straight-alpha sRGB(감마 인코딩) → linear premultiplied.
    ///
    /// contract 순서: dequantize → 선형화(EOTF) → premultiply.
    pub fn from_srgb8_straight(r: u8, g: u8, b: u8, a: u8) -> Self {
        let af = a as f32 / 255.0;
        let lr = srgb_eotf(r as f32 / 255.0);
        let lg = srgb_eotf(g as f32 / 255.0);
        let lb = srgb_eotf(b as f32 / 255.0);
        // premultiply는 선형화 후.
        Self {
            r: lr * af,
            g: lg * af,
            b: lb * af,
            a: af,
        }
    }

    /// linear premultiplied → 8bit straight-alpha sRGB.
    ///
    /// 역순: un-premultiply(alpha==0 가드) → de-linearize(OETF) → quantize.
    pub fn to_srgb8_straight(self) -> [u8; 4] {
        if self.a <= 0.0 {
            // alpha==0 가드: 0으로 나누지 않는다. 완전 투명은 색 정보 없음.
            return [0, 0, 0, 0];
        }
        let inv_a = 1.0 / self.a;
        let sr = srgb_oetf((self.r * inv_a).clamp(0.0, 1.0));
        let sg = srgb_oetf((self.g * inv_a).clamp(0.0, 1.0));
        let sb = srgb_oetf((self.b * inv_a).clamp(0.0, 1.0));
        [
            quantize_u8(sr),
            quantize_u8(sg),
            quantize_u8(sb),
            quantize_u8(self.a),
        ]
    }

    /// **디스플레이 전용** 빠른 변환 — OETF를 sqrt-간격 LUT(±1 LSB 이내)로 대체한다.
    /// 픽셀당 powf×3이 화면 갱신(수 MP)의 지배 비용이라 뷰 렌더에만 쓴다.
    /// export/PSD/골든 경로는 정확한 to_srgb8_straight를 유지(비트 계약 불변).
    pub fn to_srgb8_straight_fast(self) -> [u8; 4] {
        if self.a <= 0.0 {
            return [0, 0, 0, 0];
        }
        let inv_a = 1.0 / self.a;
        [
            srgb_oetf_u8_fast((self.r * inv_a).clamp(0.0, 1.0)),
            srgb_oetf_u8_fast((self.g * inv_a).clamp(0.0, 1.0)),
            srgb_oetf_u8_fast((self.b * inv_a).clamp(0.0, 1.0)),
            quantize_u8(self.a),
        ]
    }
}

/// linear [0,1] → sRGB u8, sqrt-간격 LUT(4096칸). sqrt 인덱싱이 어두운 영역에
/// 칸을 몰아줘 균일 4096칸보다 저역 오차가 작다(최대 ±1 LSB).
#[inline]
pub fn srgb_oetf_u8_fast(c: f32) -> u8 {
    use std::sync::OnceLock;
    static LUT: OnceLock<[u8; 4097]> = OnceLock::new();
    let lut = LUT.get_or_init(|| {
        let mut t = [0u8; 4097];
        for (i, v) in t.iter_mut().enumerate() {
            let sq = i as f32 / 4096.0;
            *v = quantize_u8(srgb_oetf(sq * sq));
        }
        t
    });
    lut[((c.max(0.0).sqrt() * 4096.0 + 0.5) as usize).min(4096)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eotf_oetf_round_trip() {
        // 전 구간 round-trip이 충분히 작은 오차로 복원되는지(piecewise 정확성).
        for i in 0..=255u32 {
            let c = i as f32 / 255.0;
            let back = srgb_oetf(srgb_eotf(c));
            assert!((back - c).abs() < 1e-5, "c={c} back={back}");
        }
    }

    #[test]
    fn eotf_is_not_naive_pow22() {
        // 저역에서 naive pow 2.2와 진짜 EOTF가 유의미하게 다름을 단언(회귀 가드).
        let c = 0.04;
        let real = srgb_eotf(c);
        let naive = c.powf(2.2);
        assert!(
            (real - naive).abs() > 1e-3,
            "EOTF가 naive pow 2.2로 퇴화하면 안 됨"
        );
    }

    #[test]
    fn dequantize_16bit_uses_32768() {
        // 흰색 16bit 샘플(32768)이 1.0으로 정규화되는지. /65535면 ~0.5로 어두워짐.
        assert_eq!(dequantize(32768, BitDepth::U16), 1.0);
        assert!((dequantize(16384, BitDepth::U16) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn blend_space_branches_on_depth() {
        assert_eq!(BlendSpace::for_bit_depth(BitDepth::U8), BlendSpace::Gamma);
        assert_eq!(BlendSpace::for_bit_depth(BitDepth::U16), BlendSpace::Gamma);
        assert_eq!(BlendSpace::for_bit_depth(BitDepth::F32), BlendSpace::Linear);
    }

    #[test]
    fn premul_after_linearize() {
        // 반투명 회색: premul 값이 (선형화된 색 * alpha)인지 확인.
        let p = LinearPremul::from_srgb8_straight(188, 188, 188, 128);
        let af = 128.0 / 255.0;
        let lin = srgb_eotf(188.0 / 255.0);
        assert!((p.r - lin * af).abs() < 1e-6);
        assert!((p.a - af).abs() < 1e-6);
    }

    #[test]
    fn transparent_round_trips_to_zero() {
        let out = LinearPremul::TRANSPARENT.to_srgb8_straight();
        assert_eq!(out, [0, 0, 0, 0]);
    }

    #[test]
    fn opaque_white_round_trip() {
        let p = LinearPremul::from_srgb8_straight(255, 255, 255, 255);
        assert_eq!(p.to_srgb8_straight(), [255, 255, 255, 255]);
    }
}
