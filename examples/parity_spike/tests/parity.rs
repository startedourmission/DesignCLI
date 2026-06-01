//! Phase 0 패리티/결정성 게이트.
//!
//! 1) CPU 정본이 결정적(같은 입력 → 비트동일)인지.
//! 2) 골든 PNG와 비트동일인지 (없으면 UPDATE_GOLDEN=1로 생성).
//! 3) 8bit 감마 vs 32bit 리니어 골든이 *다른지* (분기가 의미 있는지).
//! 4) GPU 윈도리스가 CPU 정본 대비 max-abs/SSIM 게이트 통과하는지
//!    (GPU 미가용 환경이면 skip).
//! 5) JSON save→open→픽셀 재주입→재합성이 골든과 비트동일한지 (Phase 1 직렬화 게이트).

use dcli_color::BitDepth;
use dcli_model::{fixtures::spike_scene, Document};
use std::path::PathBuf;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn cpu_rgba(depth: BitDepth) -> (u32, u32, Vec<u8>) {
    let doc = spike_scene(depth);
    (doc.width, doc.height, dcli_raster::composite(&doc).to_srgb8_rgba())
}

fn read_png(path: &PathBuf) -> Option<(u32, u32, Vec<u8>)> {
    let file = std::fs::File::open(path).ok()?;
    let dec = png::Decoder::new(file);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    Some((info.width, info.height, buf))
}

fn write_png(path: &PathBuf, w: u32, h: u32, rgba: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

fn max_abs(a: &[u8], b: &[u8]) -> u8 {
    a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0)
}

/// 그레이스케일 SSIM (global, 단일 윈도). 빠른 게이트용.
fn ssim_gray(a: &[u8], b: &[u8]) -> f64 {
    let gray = |px: &[u8]| -> Vec<f64> {
        px.chunks_exact(4)
            .map(|p| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64)
            .collect()
    };
    let ga = gray(a);
    let gb = gray(b);
    let n = ga.len() as f64;
    let ma = ga.iter().sum::<f64>() / n;
    let mb = gb.iter().sum::<f64>() / n;
    let mut va = 0.0;
    let mut vb = 0.0;
    let mut cov = 0.0;
    for (x, y) in ga.iter().zip(&gb) {
        va += (x - ma).powi(2);
        vb += (y - mb).powi(2);
        cov += (x - ma) * (y - mb);
    }
    va /= n;
    vb /= n;
    cov /= n;
    let c1 = (0.01 * 255.0_f64).powi(2);
    let c2 = (0.03 * 255.0_f64).powi(2);
    ((2.0 * ma * mb + c1) * (2.0 * cov + c2)) / ((ma * ma + mb * mb + c1) * (va + vb + c2))
}

fn golden_check(name: &str, w: u32, h: u32, rgba: &[u8]) {
    let path = golden_dir().join(name);
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        write_png(&path, w, h, rgba);
        eprintln!("updated golden: {}", path.display());
        return;
    }
    match read_png(&path) {
        Some((gw, gh, gpx)) => {
            assert_eq!((gw, gh), (w, h), "골든 크기 불일치 {name}");
            assert_eq!(gpx, rgba, "골든 비트동일 실패 {name} (UPDATE_GOLDEN=1로 갱신)");
        }
        None => panic!("골든 없음: {} — UPDATE_GOLDEN=1 cargo test 로 생성", path.display()),
    }
}

#[test]
fn cpu_is_deterministic() {
    let (_, _, a) = cpu_rgba(BitDepth::U8);
    let (_, _, b) = cpu_rgba(BitDepth::U8);
    assert_eq!(a, b, "CPU 정본이 비결정적이면 export 정본 자격 상실");
}

#[test]
fn cpu_gamma_golden() {
    let (w, h, px) = cpu_rgba(BitDepth::U8);
    golden_check("cpu_gamma_u8.png", w, h, &px);
}

#[test]
fn cpu_linear_golden() {
    let (w, h, px) = cpu_rgba(BitDepth::F32);
    golden_check("cpu_linear_f32.png", w, h, &px);
}

#[test]
fn gamma_and_linear_goldens_differ() {
    // ★최대 위험★ 분기가 진짜 다른 결과를 내는지 — 같으면 분기가 무의미.
    let (_, _, g) = cpu_rgba(BitDepth::U8);
    let (_, _, l) = cpu_rgba(BitDepth::F32);
    assert_ne!(g, l, "감마/리니어 합성 결과가 동일 — 비트깊이 분기 무력화됨");
    // 차이가 유의미한 수준인지(노이즈가 아니라 구조적 차이).
    assert!(max_abs(&g, &l) > 4, "감마/리니어 차이가 너무 작음: {}", max_abs(&g, &l));
}

#[test]
fn json_roundtrip_recomposites_to_golden() {
    // Phase 1: 구조는 JSON으로, 픽셀은 사이드카로 분리 저장됨을 검증.
    // save→open 후 픽셀을 재주입하면 재합성 결과가 원본/골든과 비트동일해야 한다.
    for depth in [BitDepth::U8, BitDepth::F32] {
        let mut original = spike_scene(depth);
        let before = dcli_raster::composite(&original).to_srgb8_rgba();

        // 1) 구조 JSON 직렬화 (픽셀 제외).
        let json = original.to_json().unwrap();
        // 2) 픽셀 사이드카를 따로 보관(실제로는 바이너리 파일).
        let pixels = original.take_pixels();

        // 3) 새 문서로 구조 로드 후 픽셀 재주입.
        let mut loaded = Document::from_json(&json).unwrap();
        assert!(loaded.pixels().is_empty(), "로드 직후엔 픽셀 비어있어야");
        loaded.set_pixels(pixels);

        // 4) 재합성 = 원본과 비트동일.
        let after = dcli_raster::composite(&loaded).to_srgb8_rgba();
        assert_eq!(before, after, "[{depth:?}] JSON 라운드트립 후 재합성 불일치");
    }
}

#[test]
fn gpu_matches_cpu_oracle() {
    // GPU 미가용 환경(CI 등)에서는 skip — softrast 정본이 진짜 게이트.
    let ctx = match dcli_gpu::GpuContext::new_headless() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("GPU 미가용, skip: {e}");
            return;
        }
    };
    for depth in [BitDepth::U8, BitDepth::F32] {
        let doc = spike_scene(depth);
        let cpu = dcli_raster::composite(&doc).to_srgb8_rgba();
        let gpu = ctx.composite(&doc).expect("gpu composite").to_srgb8_rgba();

        let m = max_abs(&cpu, &gpu);
        let s = ssim_gray(&cpu, &gpu);
        eprintln!(
            "[{:?}] adapter={} max-abs={} ssim={:.6}",
            depth, ctx.adapter_name(), m, s
        );
        // 8bit 양자화/부동소수 차이를 감안한 게이트.
        assert!(m <= 2, "[{:?}] GPU-CPU max-abs {} > 2 (8bit)", depth, m);
        assert!(s > 0.999, "[{:?}] GPU-CPU SSIM {} < 0.999", depth, s);
    }
}
