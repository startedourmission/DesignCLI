//! 문서 스냅샷 — 합성 결과를 base64 PNG로(에이전트가 "결과를 본다", 인지 루프).
//!
//! ★검증 #5 landmine★ 다운샘플은 반드시 **linear-premul 공간**에서 한다. to_srgb8(감마
//! 인코딩) 이후에 평균하면 감마 공간 평균이 되어 색이 어두워진다. 또 max_dim 미지정 시
//! 다운스케일 OFF(원본 합성 = export 비트와 동일)가 기본이라 인지 루프 신뢰도를 지킨다.

use dcli_color::LinearPremul;
use dcli_model::Document;
use dcli_tile::Surface;

/// 합성 후 (선택적) 다운샘플 → straight sRGB8 PNG 바이트.
/// 반환: (png_bytes, out_w, out_h, scaled).
pub fn snapshot_png(doc: &Document, max_dim: Option<u32>) -> anyhow::Result<(Vec<u8>, u32, u32, bool)> {
    let full = dcli_raster::composite(doc);
    let (sw, sh) = (full.width(), full.height());

    let (surface, scaled) = match max_dim {
        Some(md) if md > 0 && (sw > md || sh > md) => {
            let scale = (md as f32 / sw as f32).min(md as f32 / sh as f32);
            let ow = ((sw as f32 * scale).round() as u32).max(1);
            let oh = ((sh as f32 * scale).round() as u32).max(1);
            (downsample_linear(&full, ow, oh), true)
        }
        _ => (full, false),
    };

    let rgba = surface.to_srgb8_rgba();
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png, surface.width(), surface.height());
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()?.write_image_data(&rgba)?;
    }
    Ok((png, surface.width(), surface.height(), scaled))
}

/// linear-premul 공간 박스 평균 다운샘플(합성과 동일 공간 → 감마 오염 없음).
fn downsample_linear(src: &Surface, ow: u32, oh: u32) -> Surface {
    let (sw, sh) = (src.width(), src.height());
    let mut out = Surface::new(ow, oh);
    for oy in 0..oh {
        for ox in 0..ow {
            // 출력 픽셀이 덮는 입력 영역(박스).
            let x0 = (ox * sw) / ow;
            let x1 = (((ox + 1) * sw) / ow).max(x0 + 1).min(sw);
            let y0 = (oy * sh) / oh;
            let y1 = (((oy + 1) * sh) / oh).max(y0 + 1).min(sh);
            let (mut r, mut g, mut b, mut a) = (0.0f32, 0.0, 0.0, 0.0);
            let mut n = 0.0f32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = src.get(x, y);
                    r += p.r;
                    g += p.g;
                    b += p.b;
                    a += p.a;
                    n += 1.0;
                }
            }
            if n > 0.0 {
                out.set(ox, oy, LinearPremul { r: r / n, g: g / n, b: b / n, a: a / n });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcli_cli::dispatch::{apply_batch, Action, PixelSource};
    use dcli_color::BitDepth;
    use dcli_model::History;

    /// 단색 레이어 한 장 깔린 문서(dispatch 경유 — 외부 공개 API만 사용).
    fn doc_with_fill(w: u32, h: u32, rgba: [u8; 4]) -> History {
        let mut hist = History::new(Document::new(w, h, BitDepth::U8));
        let actions = vec![Action::AddPaintLayer {
            name: "g".into(),
            source: PixelSource::Fill { rgba },
            index: None,
            bind: None,
        }];
        let res = apply_batch(&mut hist, &actions, false);
        assert!(res.ok);
        hist
    }

    #[test]
    fn downsample_50pct_gray_not_darkened() {
        // ★landmine 회귀★ 50% 회색을 다운샘플해도 어두워지면 안 된다(linear 공간 평균).
        let h = doc_with_fill(4, 4, [128, 128, 128, 255]);
        let (_, w, hh, scaled) = snapshot_png(&h.doc, Some(2)).unwrap();
        assert_eq!((w, hh), (2, 2));
        assert!(scaled);
        let small = downsample_linear(&dcli_raster::composite(&h.doc), 2, 2);
        let px = small.get(0, 0).to_srgb8_straight();
        assert!((px[0] as i32 - 128).abs() <= 1, "다운샘플이 색을 바꿈: {}", px[0]);
    }

    #[test]
    fn no_max_dim_keeps_original_size() {
        let h = doc_with_fill(10, 6, [0, 0, 0, 0]);
        let (_, w, hh, scaled) = snapshot_png(&h.doc, None).unwrap();
        assert_eq!((w, hh), (10, 6));
        assert!(!scaled, "max_dim 미지정이면 다운스케일 OFF");
    }
}
