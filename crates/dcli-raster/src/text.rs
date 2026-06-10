//! 텍스트 래스터화 — 번들 폰트(Pretendard, OFL)를 ab_glyph로 Surface에 굽는다.
//!
//! 같은 AA 커버리지 contract(shapes.rs): 글리프 커버리지(0~1)를 linear-premul 색에
//! 곱해 source-over. 결정적(번들 폰트 단일 출처)이라 CPU 정본·wasm·CLI가 동일 비트.
//! 한글/라틴 풀커버(Pretendard-Regular). 줄바꿈은 '\n'.

use crate::shapes::{blend_px, to_linear};
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use dcli_tile::Surface;
use std::sync::OnceLock;

/// 번들 폰트 바이트(Pretendard-Regular, SIL OFL 1.1).
static FONT_BYTES: &[u8] = include_bytes!("../assets/Pretendard-Regular.otf");

fn font() -> &'static FontRef<'static> {
    static F: OnceLock<FontRef<'static>> = OnceLock::new();
    F.get_or_init(|| FontRef::try_from_slice(FONT_BYTES).expect("번들 폰트 파싱"))
}

/// 텍스트를 (x, y)에 그린다 — (x, y)는 첫 줄의 **좌상단** 기준, `size`는 px 단위.
///
/// 반환: 그린 텍스트의 (width, height) 픽셀 (레이아웃 측정값, 빈 문자열이면 0,0).
pub fn draw_text(
    s: &mut Surface,
    x: f32,
    y: f32,
    text: &str,
    size: f32,
    rgba: [u8; 4],
) -> (f32, f32) {
    if size <= 0.0 || text.is_empty() {
        return (0.0, 0.0);
    }
    let color = to_linear(rgba);
    let f = font();
    let scaled = f.as_scaled(PxScale::from(size));
    let ascent = scaled.ascent();
    let line_h = scaled.height() + scaled.line_gap();

    let mut caret_x = x;
    let mut baseline = y + ascent; // 좌상단 기준 → 첫 줄 베이스라인.
    let mut max_w = 0.0f32;
    let mut prev: Option<ab_glyph::GlyphId> = None;

    for ch in text.chars() {
        if ch == '\n' {
            max_w = max_w.max(caret_x - x);
            caret_x = x;
            baseline += line_h;
            prev = None;
            continue;
        }
        let gid = scaled.glyph_id(ch);
        if let Some(p) = prev {
            caret_x += scaled.kern(p, gid);
        }
        let glyph =
            gid.with_scale_and_position(PxScale::from(size), ab_glyph::point(caret_x, baseline));
        if let Some(og) = f.outline_glyph(glyph) {
            let b = og.px_bounds();
            og.draw(|gx, gy, cov| {
                let px = b.min.x as i32 + gx as i32;
                let py = b.min.y as i32 + gy as i32;
                if px >= 0 && py >= 0 {
                    blend_px(s, px as u32, py as u32, color, cov);
                }
            });
        }
        caret_x += scaled.h_advance(gid);
        prev = Some(gid);
    }
    max_w = max_w.max(caret_x - x);
    let total_h = baseline - y + (scaled.height() - ascent); // 마지막 줄 descent 포함.
    (max_w, total_h)
}

/// 텍스트 레이아웃 크기만 계산한다. `draw_text`와 같은 advance/line-height contract를 쓴다.
pub fn measure_text(text: &str, size: f32) -> (f32, f32) {
    if size <= 0.0 || text.is_empty() {
        return (0.0, 0.0);
    }
    let f = font();
    let scaled = f.as_scaled(PxScale::from(size));
    let ascent = scaled.ascent();
    let line_h = scaled.height() + scaled.line_gap();

    let mut caret_x = 0.0f32;
    let mut baseline = ascent;
    let mut max_w = 0.0f32;
    let mut prev: Option<ab_glyph::GlyphId> = None;

    for ch in text.chars() {
        if ch == '\n' {
            max_w = max_w.max(caret_x);
            caret_x = 0.0;
            baseline += line_h;
            prev = None;
            continue;
        }
        let gid = scaled.glyph_id(ch);
        if let Some(p) = prev {
            caret_x += scaled.kern(p, gid);
        }
        caret_x += scaled.h_advance(gid);
        prev = Some(gid);
    }
    max_w = max_w.max(caret_x);
    let total_h = baseline + (scaled.height() - ascent);
    (max_w, total_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_text_draws_pixels() {
        let mut s = Surface::new(120, 40);
        let (w, h) = draw_text(&mut s, 4.0, 4.0, "Hello", 20.0, [0, 0, 0, 255]);
        assert!(w > 20.0 && h > 10.0, "측정값 비정상: {w}x{h}");
        // 어딘가에 불투명 픽셀이 생겨야 한다.
        let any = s.pixels().iter().any(|p| p.a > 0.5);
        assert!(any, "텍스트 픽셀 없음");
    }

    #[test]
    fn korean_text_draws_pixels() {
        // 한글 글리프가 번들 폰트에 실제로 있는지(tofu 방지 핀).
        let mut s = Surface::new(200, 60);
        let (w, _) = draw_text(&mut s, 4.0, 4.0, "안녕 디자인", 24.0, [255, 0, 0, 255]);
        assert!(w > 50.0, "한글 폭 비정상: {w}");
        let count = s.pixels().iter().filter(|p| p.a > 0.5).count();
        assert!(count > 100, "한글 픽셀 너무 적음: {count} (tofu 의심)");
    }

    #[test]
    fn newline_advances_line() {
        let mut s = Surface::new(100, 100);
        let (_, h1) = draw_text(&mut s, 0.0, 0.0, "a", 20.0, [0, 0, 0, 255]);
        let mut s2 = Surface::new(100, 100);
        let (_, h2) = draw_text(&mut s2, 0.0, 0.0, "a\na", 20.0, [0, 0, 0, 255]);
        assert!(h2 > h1 * 1.5, "줄바꿈이 높이를 늘려야: {h1} → {h2}");
    }

    #[test]
    fn measure_matches_draw_layout() {
        let text = "CLI\n카드뉴스";
        let measured = measure_text(text, 24.0);
        let mut s = Surface::new(300, 120);
        let drawn = draw_text(&mut s, 0.0, 0.0, text, 24.0, [0, 0, 0, 255]);
        assert_eq!(measured, drawn);
    }

    #[test]
    fn deterministic() {
        let render = || {
            let mut s = Surface::new(100, 40);
            draw_text(&mut s, 2.0, 2.0, "Dx한글", 18.0, [10, 20, 30, 255]);
            s.to_srgb8_rgba()
        };
        assert_eq!(render(), render(), "텍스트 래스터가 비결정적");
    }
}
