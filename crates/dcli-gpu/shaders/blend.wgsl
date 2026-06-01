// GPU 합성 셰이더. dcli-raster(CPU 정본)와 **동일한 수학**을 복제한다.
// parity 테스트가 CPU 대비 SSIM/max-abs ~1e-4 게이트로 일치를 강제한다.
//
// 입력: linear-premul f32 레이어들을 storage 텍스처 배열 대신 단일 패스에서
// 누적하기 위해, 레이어를 텍스처로 바인딩하고 fragment에서 순서대로 합성한다.
// Phase 0 단순화: 최대 N개 레이어를 텍스처 배열로 받아 셰이더 내 루프 합성.

const MAX_LAYERS: u32 = 8u;

struct LayerMeta {
    blend: u32,     // 0=Normal,1=Multiply,2=Screen
    opacity: f32,
    _pad0: f32,
    _pad1: f32,
};

struct Uniforms {
    layer_count: u32,
    blend_space: u32, // 0=Gamma, 1=Linear
    _pad0: u32,
    _pad1: u32,
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
    } else { // Normal
        bs = src.rgb;
    }
    let sa = src.a;
    let out_a = sa + dst.a * (1.0 - sa);
    let rgb = bs + dst.rgb * (1.0 - sa);
    return vec4<f32>(rgb, out_a);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    for (var i: u32 = 0u; i < u.layer_count; i = i + 1u) {
        let lm = u.layers[i];
        var src = textureSampleLevel(layer_tex, samp, in.uv, i32(i), 0.0);
        // 레이어 opacity 적용 (premul 불변식 유지).
        src = src * lm.opacity;
        if (u.blend_space == 1u) {
            acc = blend_in_linear(acc, src, lm.blend);
        } else {
            acc = blend_in_gamma(acc, src, lm.blend);
        }
    }
    return acc;
}
