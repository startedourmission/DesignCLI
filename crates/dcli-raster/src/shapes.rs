//! 도형 래스터화 — 사각형/타원/선을 안티에일리어싱으로 Surface에 그린다.
//!
//! ★색 contract★ (gamma-vs-linear-landmine): AA는 픽셀 커버리지(0~1)를 색의 alpha에
//! 곱하는 방식인데, 이 곱셈은 반드시 **linear-premul 공간**에서 한다. premul 값에
//! 커버리지를 곱하면 (linear_rgb*a*cov, a*cov)이 되어 부분 커버 픽셀의 색이 정확하다.
//! straight 색을 먼저 srgb_eotf로 선형화한 뒤 premul하므로 contract를 지킨다.
//!
//! 그린 결과는 기존 Surface 위에 source-over(premul)로 누적된다 — 빈(투명) Surface에
//! 그리면 그 자체가 도형 레이어가 되고, 합성기가 문서 blend로 다시 합친다.

use dcli_color::LinearPremul;
use dcli_tile::Surface;

/// straight sRGB8 색 한 픽셀을 커버리지(0~1)만큼 dst 위에 source-over.
#[inline]
fn over_coverage(dst: LinearPremul, color: LinearPremul, coverage: f32) -> LinearPremul {
    let c = coverage.clamp(0.0, 1.0);
    if c <= 0.0 {
        return dst;
    }
    // color는 이미 linear-premul. 커버리지를 premul 성분 전체에 곱한다.
    let sr = color.r * c;
    let sg = color.g * c;
    let sb = color.b * c;
    let sa = color.a * c;
    let inv = 1.0 - sa;
    LinearPremul {
        r: sr + dst.r * inv,
        g: sg + dst.g * inv,
        b: sb + dst.b * inv,
        a: sa + dst.a * inv,
    }
}

#[inline]
pub(crate) fn blend_px(s: &mut Surface, x: u32, y: u32, color: LinearPremul, coverage: f32) {
    if x < s.width() && y < s.height() {
        let cur = s.get(x, y);
        s.set(x, y, over_coverage(cur, color, coverage));
    }
}

/// straight sRGB8 RGBA → linear-premul.
pub(crate) fn to_linear(rgba: [u8; 4]) -> LinearPremul {
    LinearPremul::from_srgb8_straight(rgba[0], rgba[1], rgba[2], rgba[3])
}

/// 채워진 사각형(축 정렬). [x, x+w) × [y, y+h). 부분 픽셀은 커버리지로 AA.
///
/// 좌표는 f32 → 가장자리가 픽셀 경계에 안 맞으면 부분 커버리지가 생긴다.
pub fn fill_rect(s: &mut Surface, x: f32, y: f32, w: f32, h: f32, rgba: [u8; 4]) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let color = to_linear(rgba);
    let (x0, y0, x1, y1) = (x, y, x + w, y + h);
    let px0 = x0.floor().max(0.0) as u32;
    let py0 = y0.floor().max(0.0) as u32;
    let px1 = (x1.ceil() as i64).clamp(0, s.width() as i64) as u32;
    let py1 = (y1.ceil() as i64).clamp(0, s.height() as i64) as u32;
    for py in py0..py1 {
        let cy = pixel_coverage_1d(py as f32, y0, y1);
        for px in px0..px1 {
            let cx = pixel_coverage_1d(px as f32, x0, x1);
            blend_px(s, px, py, color, cx * cy);
        }
    }
}

/// 1D에서 픽셀 [p, p+1)이 구간 [a, b)에 덮이는 길이(0~1).
#[inline]
fn pixel_coverage_1d(p: f32, a: f32, b: f32) -> f32 {
    let lo = p.max(a);
    let hi = (p + 1.0).min(b);
    (hi - lo).clamp(0.0, 1.0)
}

/// 픽셀 중심 (px+0.5, py+0.5)의 타원 내부 커버리지 근사(경계 1px AA). 0~1.
#[inline]
fn ellipse_coverage(px: u32, py: u32, cx: f32, cy: f32, rx: f32, ry: f32) -> f32 {
    if rx <= 0.0 || ry <= 0.0 {
        return 0.0;
    }
    let sx = (px as f32 + 0.5 - cx) / rx;
    let sy = (py as f32 + 0.5 - cy) / ry;
    let dist = (sx * sx + sy * sy).sqrt(); // 1.0이 경계.
    let grad = 0.5 / rx.min(ry); // 대략적 1px 폭.
    ((1.0 - dist) / grad + 0.5).clamp(0.0, 1.0)
}

/// 채워진 타원 — 중심 (cx, cy), 반지름 (rx, ry). 가장자리 AA(거리 기반).
pub fn fill_ellipse(s: &mut Surface, cx: f32, cy: f32, rx: f32, ry: f32, rgba: [u8; 4]) {
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let color = to_linear(rgba);
    let x0 = ((cx - rx).floor().max(0.0)) as u32;
    let y0 = ((cy - ry).floor().max(0.0)) as u32;
    let x1 = (((cx + rx).ceil() as i64).clamp(0, s.width() as i64)) as u32;
    let y1 = (((cy + ry).ceil() as i64).clamp(0, s.height() as i64)) as u32;
    for py in y0..y1 {
        for px in x0..x1 {
            blend_px(s, px, py, color, ellipse_coverage(px, py, cx, cy, rx, ry));
        }
    }
}

/// 테두리 사각형 — 외곽선만, 두께 `width`(안쪽으로). 채움 없음.
///
/// 겹치지 않는 4개 스트립(상/하/좌/우)으로 분해해 fill_rect의 AA를 그대로 재사용한다
/// (모서리 이중 블렌딩 없음). 두께가 절반 이상이면 채움과 동일.
pub fn stroke_rect(s: &mut Surface, x: f32, y: f32, w: f32, h: f32, width: f32, rgba: [u8; 4]) {
    if w <= 0.0 || h <= 0.0 || width <= 0.0 {
        return;
    }
    let t = width;
    if t * 2.0 >= w.min(h) {
        return fill_rect(s, x, y, w, h, rgba);
    }
    fill_rect(s, x, y, w, t, rgba); // 상
    fill_rect(s, x, y + h - t, w, t, rgba); // 하
    fill_rect(s, x, y + t, t, h - 2.0 * t, rgba); // 좌
    fill_rect(s, x + w - t, y + t, t, h - 2.0 * t, rgba); // 우
}

/// 테두리 타원 — 링(외곽 타원 − 안쪽 타원), 두께 `width`(안쪽으로).
///
/// 커버리지를 한 패스에서 (outer − inner)로 계산해 이중 블렌딩을 피한다.
pub fn stroke_ellipse(
    s: &mut Surface,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    width: f32,
    rgba: [u8; 4],
) {
    if rx <= 0.0 || ry <= 0.0 || width <= 0.0 {
        return;
    }
    let (irx, iry) = (rx - width, ry - width);
    if irx <= 0.0 || iry <= 0.0 {
        return fill_ellipse(s, cx, cy, rx, ry, rgba);
    }
    let color = to_linear(rgba);
    let x0 = ((cx - rx).floor().max(0.0)) as u32;
    let y0 = ((cy - ry).floor().max(0.0)) as u32;
    let x1 = (((cx + rx).ceil() as i64).clamp(0, s.width() as i64)) as u32;
    let y1 = (((cy + ry).ceil() as i64).clamp(0, s.height() as i64)) as u32;
    for py in y0..y1 {
        for px in x0..x1 {
            let outer = ellipse_coverage(px, py, cx, cy, rx, ry);
            let inner = ellipse_coverage(px, py, cx, cy, irx, iry);
            blend_px(s, px, py, color, (outer - inner).clamp(0.0, 1.0));
        }
    }
}

/// 모서리 둥근 채움 사각형 — 코너 반지름 `radius`. 표준 rounded-box SDF로 1px AA.
pub fn fill_rounded_rect(
    s: &mut Surface,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    rgba: [u8; 4],
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let r = radius.clamp(0.0, w.min(h) * 0.5);
    if r <= 0.0 {
        return fill_rect(s, x, y, w, h, rgba);
    }
    let color = to_linear(rgba);
    let (ccx, ccy) = (x + w * 0.5, y + h * 0.5); // 중심
    let (hx, hy) = (w * 0.5 - r, h * 0.5 - r); // 코너 중심까지 반치수
    let px0 = x.floor().max(0.0) as u32;
    let py0 = y.floor().max(0.0) as u32;
    let px1 = (((x + w).ceil() as i64).clamp(0, s.width() as i64)) as u32;
    let py1 = (((y + h).ceil() as i64).clamp(0, s.height() as i64)) as u32;
    for py in py0..py1 {
        for px in px0..px1 {
            // rounded-box 부호화 거리: 음수=내부. 경계 1px AA.
            let qx = (px as f32 + 0.5 - ccx).abs() - hx;
            let qy = (py as f32 + 0.5 - ccy).abs() - hy;
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let d = outside + qx.max(qy).min(0.0) - r;
            let coverage = (0.5 - d).clamp(0.0, 1.0);
            blend_px(s, px, py, color, coverage);
        }
    }
}

/// 선분 — (x0,y0)→(x1,y1), 두께 width. 둥근 끝(capsule 거리)으로 AA.
pub fn stroke_line(s: &mut Surface, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, rgba: [u8; 4]) {
    if width <= 0.0 {
        return;
    }
    let color = to_linear(rgba);
    let half = width * 0.5;
    let minx = (x0.min(x1) - half).floor().max(0.0) as u32;
    let miny = (y0.min(y1) - half).floor().max(0.0) as u32;
    let maxx = (((x0.max(x1) + half).ceil() as i64).clamp(0, s.width() as i64)) as u32;
    let maxy = (((y0.max(y1) + half).ceil() as i64).clamp(0, s.height() as i64)) as u32;
    for py in miny..maxy {
        for px in minx..maxx {
            let d = dist_point_segment(px as f32 + 0.5, py as f32 + 0.5, x0, y0, x1, y1);
            // 경계에서 1픽셀 폭 AA.
            let coverage = (half - d + 0.5).clamp(0.0, 1.0);
            blend_px(s, px, py, color, coverage);
        }
    }
}

/// 점 (px,py)에서 선분 (ax,ay)-(bx,by)까지의 최단 거리.
#[inline]
fn dist_point_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    if len2 <= f32::EPSILON {
        let ex = px - ax;
        let ey = py - ay;
        return (ex * ex + ey * ey).sqrt();
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0);
    let projx = ax + t * dx;
    let projy = ay + t * dy;
    let ex = px - projx;
    let ey = py - projy;
    (ex * ex + ey * ey).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_aligned_rect_is_opaque() {
        // 픽셀 경계에 딱 맞는 사각형은 내부가 완전 불투명.
        let mut s = Surface::new(10, 10);
        fill_rect(&mut s, 2.0, 2.0, 4.0, 4.0, [255, 0, 0, 255]);
        let px = s.get(3, 3).to_srgb8_straight();
        assert_eq!(px, [255, 0, 0, 255], "내부 픽셀은 불투명 빨강");
        // 바깥은 투명.
        assert_eq!(s.get(0, 0).to_srgb8_straight(), [0, 0, 0, 0]);
    }

    #[test]
    fn half_pixel_rect_edge_is_partial() {
        // 0.5px 시작 → 첫 열은 50% 커버리지(가장자리 AA).
        let mut s = Surface::new(10, 10);
        fill_rect(&mut s, 2.5, 2.0, 4.0, 4.0, [255, 0, 0, 255]);
        let a = s.get(2, 3).to_srgb8_straight()[3];
        assert!(a > 100 && a < 160, "가장자리 alpha ~50%: {a}");
    }

    #[test]
    fn ellipse_center_opaque_corner_transparent() {
        let mut s = Surface::new(20, 20);
        fill_ellipse(&mut s, 10.0, 10.0, 8.0, 8.0, [0, 128, 255, 255]);
        assert_eq!(s.get(10, 10).to_srgb8_straight()[3], 255, "중심 불투명");
        assert_eq!(s.get(0, 0).to_srgb8_straight()[3], 0, "모서리 투명");
    }

    #[test]
    fn line_draws_along_path() {
        let mut s = Surface::new(20, 20);
        stroke_line(&mut s, 2.0, 10.0, 18.0, 10.0, 3.0, [0, 0, 0, 255]);
        assert!(s.get(10, 10).to_srgb8_straight()[3] > 200, "선 위 픽셀 불투명");
        assert_eq!(s.get(10, 2).to_srgb8_straight()[3], 0, "선에서 먼 픽셀 투명");
    }

    #[test]
    fn stroke_rect_hollow_center() {
        // 테두리 사각형: 외곽선 위는 불투명, 내부는 투명.
        let mut s = Surface::new(20, 20);
        stroke_rect(&mut s, 2.0, 2.0, 16.0, 16.0, 2.0, [255, 0, 0, 255]);
        assert_eq!(s.get(3, 3).to_srgb8_straight()[3], 255, "테두리 위 불투명");
        assert_eq!(s.get(10, 10).to_srgb8_straight()[3], 0, "내부 투명");
        assert_eq!(s.get(0, 0).to_srgb8_straight()[3], 0, "바깥 투명");
    }

    #[test]
    fn stroke_rect_thick_degenerates_to_fill() {
        // 두께가 절반 이상이면 채움과 동일.
        let mut s = Surface::new(10, 10);
        stroke_rect(&mut s, 2.0, 2.0, 6.0, 6.0, 4.0, [0, 255, 0, 255]);
        assert_eq!(s.get(5, 5).to_srgb8_straight()[3], 255, "중심까지 채워짐");
    }

    #[test]
    fn stroke_ellipse_ring_hollow_center() {
        let mut s = Surface::new(30, 30);
        stroke_ellipse(&mut s, 15.0, 15.0, 12.0, 12.0, 3.0, [0, 0, 255, 255]);
        // 링 위(중심에서 ~10.5px 거리, 경계 12와 내부 9 사이).
        assert!(s.get(15 + 10, 15).to_srgb8_straight()[3] > 200, "링 위 불투명");
        assert_eq!(s.get(15, 15).to_srgb8_straight()[3], 0, "중심 투명");
        assert_eq!(s.get(0, 0).to_srgb8_straight()[3], 0, "바깥 투명");
    }

    #[test]
    fn rounded_rect_corners_clipped() {
        // 둥근 사각형: 모서리 픽셀은 잘려(투명) 중심·변 중앙은 불투명.
        let mut s = Surface::new(20, 20);
        fill_rounded_rect(&mut s, 2.0, 2.0, 16.0, 16.0, 6.0, [255, 0, 255, 255]);
        assert_eq!(s.get(10, 10).to_srgb8_straight()[3], 255, "중심 불투명");
        assert!(s.get(10, 2).to_srgb8_straight()[3] > 200, "변 중앙 불투명");
        assert_eq!(s.get(2, 2).to_srgb8_straight()[3], 0, "코너 잘림(투명)");
    }

    #[test]
    fn coverage_color_not_corrupted() {
        // 50% 커버 흰색을 투명 위에 → straight 흰색 + 50% alpha(색 안 변함).
        let mut s = Surface::new(4, 4);
        // 한 픽셀을 정확히 50% 덮는 사각형.
        fill_rect(&mut s, 1.0, 1.0, 0.5, 1.0, [255, 255, 255, 255]);
        let px = s.get(1, 1).to_srgb8_straight();
        assert!((px[3] as i32 - 128).abs() <= 4, "alpha ~50%: {}", px[3]);
        // un-premultiply 후 색은 흰색 유지.
        assert!(px[0] > 240 && px[1] > 240 && px[2] > 240, "색 보존: {px:?}");
    }
}
