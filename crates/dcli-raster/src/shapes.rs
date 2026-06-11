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
    let color = to_linear(rgba);
    fill_rect_with(s, x, y, w, h, &|_, _| color);
}

/// 픽셀별 색 콜백 버전(그라데이션 채움) — 커버리지 수학은 단색과 동일.
pub fn fill_rect_with(
    s: &mut Surface,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color_at: &dyn Fn(f32, f32) -> LinearPremul,
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let (x0, y0, x1, y1) = (x, y, x + w, y + h);
    let px0 = x0.floor().max(0.0) as u32;
    let py0 = y0.floor().max(0.0) as u32;
    let px1 = (x1.ceil() as i64).clamp(0, s.width() as i64) as u32;
    let py1 = (y1.ceil() as i64).clamp(0, s.height() as i64) as u32;
    for py in py0..py1 {
        let cy = pixel_coverage_1d(py as f32, y0, y1);
        for px in px0..px1 {
            let cx = pixel_coverage_1d(px as f32, x0, x1);
            blend_px(s, px, py, color_at(px as f32 + 0.5, py as f32 + 0.5), cx * cy);
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
    let color = to_linear(rgba);
    fill_ellipse_with(s, cx, cy, rx, ry, &|_, _| color);
}

/// 픽셀별 색 콜백 버전(그라데이션 채움).
pub fn fill_ellipse_with(
    s: &mut Surface,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    color_at: &dyn Fn(f32, f32) -> LinearPremul,
) {
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let x0 = ((cx - rx).floor().max(0.0)) as u32;
    let y0 = ((cy - ry).floor().max(0.0)) as u32;
    let x1 = (((cx + rx).ceil() as i64).clamp(0, s.width() as i64)) as u32;
    let y1 = (((cy + ry).ceil() as i64).clamp(0, s.height() as i64)) as u32;
    for py in y0..y1 {
        for px in x0..x1 {
            let cov = ellipse_coverage(px, py, cx, cy, rx, ry);
            if cov > 0.0 {
                blend_px(s, px, py, color_at(px as f32 + 0.5, py as f32 + 0.5), cov);
            }
        }
    }
}

/// 테두리 사각형 — 외곽선만, 두께 `width`(안쪽으로). 채움 없음.
///
/// 겹치지 않는 4개 스트립(상/하/좌/우)으로 분해해 fill_rect의 AA를 그대로 재사용한다
/// (모서리 이중 블렌딩 없음). 두께가 절반 이상이면 채움과 동일.
pub fn stroke_rect(s: &mut Surface, x: f32, y: f32, w: f32, h: f32, width: f32, rgba: [u8; 4]) {
    // 한 패스 링 커버리지(outer − inner). 예전 4-스트립(fill_rect×4) 방식은 분수 좌표에서
    // 가로/세로 변이 각자 AA되어 이음새에 반투명 이격이 생겼다(뷰 벡터 재래스터에서 가시).
    stroke_rounded_rect(s, x, y, w, h, 0.0, width, rgba)
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
    let color = to_linear(rgba);
    fill_rounded_rect_with(s, x, y, w, h, radius, &|_, _| color);
}

/// 픽셀별 색 콜백 버전(그라데이션 채움).
pub fn fill_rounded_rect_with(
    s: &mut Surface,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    color_at: &dyn Fn(f32, f32) -> LinearPremul,
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let r = radius.clamp(0.0, w.min(h) * 0.5);
    if r <= 0.0 {
        return fill_rect_with(s, x, y, w, h, color_at);
    }
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
            if coverage > 0.0 {
                blend_px(s, px, py, color_at(px as f32 + 0.5, py as f32 + 0.5), coverage);
            }
        }
    }
}

#[inline]
fn rounded_rect_coverage(px: u32, py: u32, x: f32, y: f32, w: f32, h: f32, radius: f32) -> f32 {
    if w <= 0.0 || h <= 0.0 {
        return 0.0;
    }
    let r = radius.clamp(0.0, w.min(h) * 0.5);
    if r <= 0.0 {
        let cx = pixel_coverage_1d(px as f32, x, x + w);
        let cy = pixel_coverage_1d(py as f32, y, y + h);
        return cx * cy;
    }
    let (ccx, ccy) = (x + w * 0.5, y + h * 0.5);
    let (hx, hy) = (w * 0.5 - r, h * 0.5 - r);
    let qx = (px as f32 + 0.5 - ccx).abs() - hx;
    let qy = (py as f32 + 0.5 - ccy).abs() - hy;
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    let d = outside + qx.max(qy).min(0.0) - r;
    (0.5 - d).clamp(0.0, 1.0)
}

/// 둥근 사각형 테두리 — 바깥 rounded box에서 안쪽 rounded box를 뺀 링.
pub fn stroke_rounded_rect(
    s: &mut Surface,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    width: f32,
    rgba: [u8; 4],
) {
    if w <= 0.0 || h <= 0.0 || width <= 0.0 {
        return;
    }
    let t = width;
    if t * 2.0 >= w.min(h) {
        return fill_rounded_rect(s, x, y, w, h, radius, rgba);
    }
    let color = to_linear(rgba);
    let px0 = x.floor().max(0.0) as u32;
    let py0 = y.floor().max(0.0) as u32;
    let px1 = (((x + w).ceil() as i64).clamp(0, s.width() as i64)) as u32;
    let py1 = (((y + h).ceil() as i64).clamp(0, s.height() as i64)) as u32;
    for py in py0..py1 {
        for px in px0..px1 {
            let outer = rounded_rect_coverage(px, py, x, y, w, h, radius);
            let inner = rounded_rect_coverage(
                px,
                py,
                x + t,
                y + t,
                w - t * 2.0,
                h - t * 2.0,
                (radius - t).max(0.0),
            );
            blend_px(s, px, py, color, (outer - inner).clamp(0.0, 1.0));
        }
    }
}

/// 부드러운 그림자 — rounded-box SDF를 feather 폭의 smoothstep으로 풀어낸다.
/// feather=0이면 일반 rounded rect와 동일(1px AA).
pub fn fill_shadow(
    s: &mut Surface,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    feather: f32,
    rgba: [u8; 4],
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let f = feather.max(0.0);
    if f <= 0.0 {
        return fill_rounded_rect(s, x, y, w, h, radius, rgba);
    }
    let color = to_linear(rgba);
    let r = radius.clamp(0.0, w.min(h) * 0.5);
    let (ccx, ccy) = (x + w * 0.5, y + h * 0.5);
    let (hx, hy) = (w * 0.5 - r, h * 0.5 - r);
    let px0 = (x - f).floor().max(0.0) as u32;
    let py0 = (y - f).floor().max(0.0) as u32;
    let px1 = (((x + w + f).ceil() as i64).clamp(0, s.width() as i64)) as u32;
    let py1 = (((y + h + f).ceil() as i64).clamp(0, s.height() as i64)) as u32;
    for py in py0..py1 {
        for px in px0..px1 {
            let qx = (px as f32 + 0.5 - ccx).abs() - hx;
            let qy = (py as f32 + 0.5 - ccy).abs() - hy;
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let d = outside + qx.max(qy).min(0.0) - r;
            // smoothstep: 경계(d=0) 안팎 ±f/2 폭으로 부드럽게.
            let t = (0.5 - d / f).clamp(0.0, 1.0);
            let cov = t * t * (3.0 - 2.0 * t);
            if cov > 0.0 {
                blend_px(s, px, py, color, cov);
            }
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

/// stops(straight sRGB8, at 오름차순)를 linear-premul로 변환한다.
///
/// ★색 contract★: 그라디언트 색 보간은 반드시 linear-premul 공간에서 한다 —
/// 감마 공간 보간은 중간 톤이 어두워지는 고전적 함정(gamma-vs-linear-landmine).
pub fn stops_to_linear(stops: &[(f32, [u8; 4])]) -> Vec<(f32, LinearPremul)> {
    stops
        .iter()
        .map(|(at, rgba)| (*at, to_linear(*rgba)))
        .collect()
}

/// t(0~1 권장, 범위 밖 clamp)에 해당하는 보간 색. stops는 at 오름차순 가정.
///
/// 첫 stop 이전/마지막 stop 이후는 끝 색으로 clamp. stop 사이는 성분별 선형 보간
/// (linear-premul이므로 premul 성분을 그대로 lerp하면 합성과 일관된 결과).
pub fn gradient_color_at(stops: &[(f32, LinearPremul)], t: f32) -> LinearPremul {
    let first = &stops[0];
    if t <= first.0 {
        return first.1;
    }
    let last = &stops[stops.len() - 1];
    if t >= last.0 {
        return last.1;
    }
    // 인접 stop 쌍을 찾아 구간 내 비율로 lerp.
    for pair in stops.windows(2) {
        let (a0, c0) = pair[0];
        let (a1, c1) = pair[1];
        if t <= a1 {
            let span = a1 - a0;
            let f = if span <= f32::EPSILON {
                0.0
            } else {
                (t - a0) / span
            };
            return LinearPremul {
                r: c0.r + (c1.r - c0.r) * f,
                g: c0.g + (c1.g - c0.g) * f,
                b: c0.b + (c1.b - c0.b) * f,
                a: c0.a + (c1.a - c0.a) * f,
            };
        }
    }
    last.1
}

/// 선형 그라디언트로 표면 전체를 채운다 — (x0,y0)→(x1,y1) 축 위 투영이 t.
///
/// 픽셀 중심을 축에 투영해 t∈[0,1]로 clamp. 클리핑은 합성에 맡긴다(전면 채움).
/// 축 길이가 0이면 첫 stop 색으로 단색 채움.
pub fn fill_linear_gradient(
    s: &mut Surface,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    stops: &[(f32, [u8; 4])],
) {
    if stops.is_empty() {
        return;
    }
    let lin = stops_to_linear(stops);
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len2 = dx * dx + dy * dy;
    for py in 0..s.height() {
        for px in 0..s.width() {
            let t = if len2 <= f32::EPSILON {
                0.0
            } else {
                // 픽셀 중심을 축에 투영한 비율.
                (((px as f32 + 0.5 - x0) * dx + (py as f32 + 0.5 - y0) * dy) / len2).clamp(0.0, 1.0)
            };
            blend_px(s, px, py, gradient_color_at(&lin, t), 1.0);
        }
    }
}

/// 원형(방사형) 그라디언트로 표면 전체를 채운다 — 중심 거리/radius가 t.
///
/// radius 밖은 t=1로 clamp(마지막 stop 색). radius가 0 이하이면 전부 t=1.
pub fn fill_radial_gradient(
    s: &mut Surface,
    cx: f32,
    cy: f32,
    radius: f32,
    stops: &[(f32, [u8; 4])],
) {
    if stops.is_empty() {
        return;
    }
    let lin = stops_to_linear(stops);
    for py in 0..s.height() {
        for px in 0..s.width() {
            let ex = px as f32 + 0.5 - cx;
            let ey = py as f32 + 0.5 - cy;
            let dist = (ex * ex + ey * ey).sqrt();
            let t = if radius <= f32::EPSILON {
                1.0
            } else {
                (dist / radius).clamp(0.0, 1.0)
            };
            blend_px(s, px, py, gradient_color_at(&lin, t), 1.0);
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
        assert!(
            s.get(10, 10).to_srgb8_straight()[3] > 200,
            "선 위 픽셀 불투명"
        );
        assert_eq!(
            s.get(10, 2).to_srgb8_straight()[3],
            0,
            "선에서 먼 픽셀 투명"
        );
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
        assert!(
            s.get(15 + 10, 15).to_srgb8_straight()[3] > 200,
            "링 위 불투명"
        );
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
    fn linear_gradient_black_to_white_midpoint() {
        // 17px 폭, 축 0.5→16.5: 픽셀 0 중심 t=0, 픽셀 8 중심 t=0.5, 픽셀 16 중심 t=1.
        let mut s = Surface::new(17, 1);
        let stops = [(0.0_f32, [0u8, 0, 0, 255]), (1.0, [255, 255, 255, 255])];
        fill_linear_gradient(&mut s, 0.5, 0.0, 16.5, 0.0, &stops);
        assert_eq!(s.get(0, 0).to_srgb8_straight(), [0, 0, 0, 255], "t=0 검정");
        assert_eq!(
            s.get(16, 0).to_srgb8_straight(),
            [255, 255, 255, 255],
            "t=1 흰색"
        );
        // t=0.5 — ★linear 공간 보간★: linear 0.5 → sRGB8 ≈ 188 (감마 보간이면 128).
        let mid = s.get(8, 0).to_srgb8_straight();
        assert!(
            (mid[0] as i32 - 188).abs() <= 2,
            "중간값은 linear 보간(≈188): {mid:?}"
        );
        assert_eq!(mid[0], mid[1], "회색(채널 동일)");
        assert_eq!(mid[3], 255, "불투명");
    }

    #[test]
    fn radial_gradient_center_and_edge() {
        // 중심 (8.5, 8.5) = 픽셀 (8,8) 중심, radius 8 → 픽셀 (16,8) 중심이 정확히 t=1.
        let mut s = Surface::new(17, 17);
        let stops = [(0.0_f32, [0u8, 0, 0, 255]), (1.0, [255, 255, 255, 255])];
        fill_radial_gradient(&mut s, 8.5, 8.5, 8.0, &stops);
        assert_eq!(
            s.get(8, 8).to_srgb8_straight(),
            [0, 0, 0, 255],
            "중심 t=0 검정"
        );
        assert_eq!(
            s.get(16, 8).to_srgb8_straight(),
            [255, 255, 255, 255],
            "가장자리 t=1 흰색"
        );
        // radius 밖(모서리)은 t=1로 clamp → 흰색.
        assert_eq!(
            s.get(0, 0).to_srgb8_straight(),
            [255, 255, 255, 255],
            "범위 밖 clamp"
        );
    }

    #[test]
    fn gradient_stops_clamped_outside_range() {
        // stop 범위가 [0.25, 0.75]이면 그 밖의 t는 끝 색으로 clamp.
        let mut s = Surface::new(17, 1);
        let stops = [(0.25_f32, [255u8, 0, 0, 255]), (0.75, [0, 0, 255, 255])];
        fill_linear_gradient(&mut s, 0.5, 0.0, 16.5, 0.0, &stops);
        assert_eq!(
            s.get(0, 0).to_srgb8_straight(),
            [255, 0, 0, 255],
            "t=0 → 첫 stop"
        );
        assert_eq!(
            s.get(16, 0).to_srgb8_straight(),
            [0, 0, 255, 255],
            "t=1 → 마지막 stop"
        );
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
