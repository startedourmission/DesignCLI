// GPU 합성 셰이더. dcli-raster(CPU 정본)와 **동일한 수학**을 복제한다.
// parity 테스트가 CPU 대비 SSIM/max-abs ~1e-4 게이트로 일치를 강제한다.
//
// 입력: linear-premul f32 레이어들을 storage 텍스처 배열 대신 단일 패스에서
// 누적하기 위해, 레이어를 텍스처로 바인딩하고 fragment에서 순서대로 합성한다.
// Phase 0 단순화: 최대 N개 레이어를 텍스처 배열로 받아 셰이더 내 루프 합성.

const MAX_LAYERS: u32 = 8u;

struct LayerMeta {
    blend: u32,     // 0=Normal,1=Multiply,2=Screen,3=Darken,4=Lighten,5=Overlay,6=Difference
    opacity: f32,
    offset_x: i32,  // 캔버스 평행이동 (dx,dy) 정수 픽셀 (CPU composite_layer와 동일)
    offset_y: i32,
    scale_x: f32,   // 비파괴 스케일. (1,1)+sin0+cos1 = identity → 정수 시프트 경로
    scale_y: f32,
    rot_sin: f32,   // 회전 sin/cos (CPU 선계산 — 동일 입력으로 parity 유지)
    rot_cos: f32,
};

struct Uniforms {
    layer_count: u32,
    blend_space: u32, // 0=Gamma, 1=Linear
    doc_w: u32,
    doc_h: u32,
    layers: array<LayerMeta, MAX_LAYERS>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var layer_tex: texture_2d_array<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // 풀스크린 삼각형.
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    var out: VsOut;
    let xy = p[vi];
    out.pos = vec4<f32>(xy, 0.0, 1.0);
    // uv: 좌상단 원점(텍스처 좌표). CPU는 row-major top-left.
    out.uv = vec2<f32>((xy.x + 1.0) * 0.5, (1.0 - (xy.y + 1.0) * 0.5));
    return out;
}

// ---- sRGB 전달함수 (dcli-color와 동일 piecewise) ----
fn srgb_eotf(c: f32) -> f32 {
    if (c <= 0.040448237) {
        return c / 12.92;
    }
    return pow((c + 0.055) / 1.055, 2.4);
}

fn srgb_oetf(c: f32) -> f32 {
    if (c <= 0.0031308) {
        return c * 12.92;
    }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}

fn screen1(a: f32, b: f32) -> f32 {
    return 1.0 - (1.0 - a) * (1.0 - b);
}

// Overlay 성분 공식 (dcli-raster::overlay와 1:1): dst가 어두우면 2ds, 밝으면 1−2(1−d)(1−s).
fn overlay1(d: f32, s: f32) -> f32 {
    if (d <= 0.5) {
        return 2.0 * d * s;
    }
    return 1.0 - 2.0 * (1.0 - d) * (1.0 - s);
}

// linear-premul straight 변환 (alpha==0 가드).
fn to_straight_gamma(p: vec4<f32>) -> vec3<f32> {
    if (p.a <= 0.0) {
        return vec3<f32>(0.0);
    }
    let inv = 1.0 / p.a;
    return vec3<f32>(
        srgb_oetf(clamp(p.r * inv, 0.0, 1.0)),
        srgb_oetf(clamp(p.g * inv, 0.0, 1.0)),
        srgb_oetf(clamp(p.b * inv, 0.0, 1.0)),
    );
}

// 감마 공간 블렌드 (dcli-raster::blend_in_gamma와 1:1).
fn blend_in_gamma(dst: vec4<f32>, src: vec4<f32>, blend: u32) -> vec4<f32> {
    let dg = to_straight_gamma(dst);
    let sg = to_straight_gamma(src);
    var blended_gamma: vec3<f32>;
    if (blend == 1u) { // Multiply
        blended_gamma = dg * sg;
    } else if (blend == 2u) { // Screen
        blended_gamma = vec3<f32>(
            screen1(dg.x, sg.x), screen1(dg.y, sg.y), screen1(dg.z, sg.z));
    } else if (blend == 3u) { // Darken
        blended_gamma = min(dg, sg);
    } else if (blend == 4u) { // Lighten
        blended_gamma = max(dg, sg);
    } else if (blend == 5u) { // Overlay
        blended_gamma = vec3<f32>(
            overlay1(dg.x, sg.x), overlay1(dg.y, sg.y), overlay1(dg.z, sg.z));
    } else if (blend == 6u) { // Difference
        blended_gamma = abs(dg - sg);
    } else { // Normal
        blended_gamma = sg;
    }
    let blended_lin = vec3<f32>(
        srgb_eotf(blended_gamma.x),
        srgb_eotf(blended_gamma.y),
        srgb_eotf(blended_gamma.z),
    );
    let sa = src.a;
    let out_a = sa + dst.a * (1.0 - sa);
    let rgb = blended_lin * sa + dst.rgb * (1.0 - sa);
    return vec4<f32>(rgb, out_a);
}

// linear 공간 블렌드 (dcli-raster::blend_in_linear와 1:1, premul 성분 직접).
fn blend_in_linear(dst: vec4<f32>, src: vec4<f32>, blend: u32) -> vec4<f32> {
    var bs: vec3<f32>;
    if (blend == 1u) { // Multiply (premul 성분)
        bs = dst.rgb * src.rgb;
    } else if (blend == 2u) { // Screen
        bs = vec3<f32>(
            screen1(dst.r, src.r), screen1(dst.g, src.g), screen1(dst.b, src.b));
    } else if (blend == 3u) { // Darken (premul 성분 — CPU blend_rgb_premul과 동일 관행)
        bs = min(dst.rgb, src.rgb);
    } else if (blend == 4u) { // Lighten
        bs = max(dst.rgb, src.rgb);
    } else if (blend == 5u) { // Overlay
        bs = vec3<f32>(
            overlay1(dst.r, src.r), overlay1(dst.g, src.g), overlay1(dst.b, src.b));
    } else if (blend == 6u) { // Difference
        bs = abs(dst.rgb - src.rgb);
    } else { // Normal
        bs = src.rgb;
    }
    let sa = src.a;
    let out_a = sa + dst.a * (1.0 - sa);
    let rgb = bs + dst.rgb * (1.0 - sa);
    return vec4<f32>(rgb, out_a);
}

// 경계 밖 = 투명 탭(CPU tap()과 동일).
fn tap(x: i32, y: i32, layer: i32, dw: i32, dh: i32) -> vec4<f32> {
    if (x < 0 || y < 0 || x >= dw || y >= dh) {
        return vec4<f32>(0.0);
    }
    return textureLoad(layer_tex, vec2<i32>(x, y), layer, 0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    // 목적지 픽셀 좌표(정수, top-left 원점). builtin position은 픽셀 중심(+0.5)이라 floor.
    let dst_px = vec2<i32>(i32(floor(in.pos.x)), i32(floor(in.pos.y)));
    let dw = i32(u.doc_w);
    let dh = i32(u.doc_h);
    for (var i: u32 = 0u; i < u.layer_count; i = i + 1u) {
        let lm = u.layers[i];
        var src: vec4<f32>;
        if (lm.scale_x == 1.0 && lm.scale_y == 1.0 && lm.rot_sin == 0.0 && lm.rot_cos == 1.0) {
            // identity fast path: 정수 시프트(CPU composite_layer와 비트 동형).
            let sx = dst_px.x - lm.offset_x;
            let sy = dst_px.y - lm.offset_y;
            if (sx < 0 || sy < 0 || sx >= dw || sy >= dh) {
                continue;
            }
            src = textureLoad(layer_tex, vec2<i32>(sx, sy), i32(i), 0);
        } else {
            // 트랜스폼 경로: 역변환 + bilinear (CPU composite_layer_transformed와 동일 수학).
            //   q = p_dst − off − c;  r = R(−θ)q;  p_src = r/S + c.
            // 퇴화 스케일 가드 — CPU와 동일하게 스킵(0 나눗셈 → NaN/발산 방지).
            if (abs(lm.scale_x) < 1e-4 || abs(lm.scale_y) < 1e-4) {
                continue;
            }
            let c = vec2<f32>(f32(dw) * 0.5, f32(dh) * 0.5);
            let q = vec2<f32>(f32(dst_px.x) + 0.5 - f32(lm.offset_x), f32(dst_px.y) + 0.5 - f32(lm.offset_y)) - c;
            let r = vec2<f32>(lm.rot_cos * q.x + lm.rot_sin * q.y, -lm.rot_sin * q.x + lm.rot_cos * q.y);
            let p = r / vec2<f32>(lm.scale_x, lm.scale_y) + c;
            let f = p - vec2<f32>(0.5, 0.5);
            let ix = i32(floor(f.x));
            let iy = i32(floor(f.y));
            let t = f - vec2<f32>(f32(ix), f32(iy));
            let s00 = tap(ix, iy, i32(i), dw, dh);
            let s10 = tap(ix + 1, iy, i32(i), dw, dh);
            let s01 = tap(ix, iy + 1, i32(i), dw, dh);
            let s11 = tap(ix + 1, iy + 1, i32(i), dw, dh);
            src = mix(mix(s00, s10, t.x), mix(s01, s11, t.x), t.y);
            if (src.a <= 0.0) {
                continue;
            }
        }
        // 레이어 opacity 적용 (premul 불변식 유지).
        src = src * lm.opacity;
        // CPU blend_pixel과 1:1 fast path 미러(비트 패리티 유지):
        // 투명 src는 무기여, Normal×완전불투명은 dst를 src로 대체(왕복 인코딩 생략).
        if (src.a <= 0.0) {
            continue;
        }
        if (lm.blend == 0u && src.a >= 1.0) {
            acc = src;
            continue;
        }
        if (u.blend_space == 1u) {
            acc = blend_in_linear(acc, src, lm.blend);
        } else {
            acc = blend_in_gamma(acc, src, lm.blend);
        }
    }
    return acc;
}
