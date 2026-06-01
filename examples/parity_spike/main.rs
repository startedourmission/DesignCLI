//! Phase 0 디리스킹 스파이크.
//!
//! 한 UI-비종속 코어로 동일 문서를 (a) CPU 정본, (b) wgpu 윈도리스 → PNG 세 경로로
//! 렌더하고(프리뷰 창은 후속), 골든/패리티를 검증한다.
//!
//! 사용법:
//!   parity_spike --out /tmp/cpu.png              # CPU 정본 PNG (기본 U8=감마)
//!   parity_spike --gpu-headless --out /tmp/gpu.png
//!   parity_spike --linear --out /tmp/lin.png     # 32bit 리니어 경로
//!   parity_spike --diff                          # CPU vs GPU max-abs/SSIM 출력

mod preview;

use dcli_color::BitDepth;
use dcli_model::fixtures::spike_scene;

fn write_png(path: &str, w: u32, h: u32, rgba: &[u8]) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}

fn max_abs(a: &[u8], b: &[u8]) -> u8 {
    a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0)
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out: Option<String> = None;
    let mut gpu = false;
    let mut linear = false;
    let mut diff = false;
    let mut preview = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out = args.get(i).cloned();
            }
            "--gpu-headless" => gpu = true,
            "--linear" => linear = true,
            "--diff" => diff = true,
            "--preview" => preview = true,
            other => anyhow::bail!("알 수 없는 인자: {other}"),
        }
        i += 1;
    }

    let depth = if linear { BitDepth::F32 } else { BitDepth::U8 };
    let doc = spike_scene(depth);
    let (w, h) = (doc.width, doc.height);

    if preview {
        // 3번째 경로: egui-wgpu 네이티브 프리뷰 창. CPU 정본을 텍스처로 표시.
        return preview::run_preview(depth);
    }

    if diff {
        let cpu = dcli_raster::composite(&doc).to_srgb8_rgba();
        let ctx = dcli_gpu::GpuContext::new_headless()?;
        let gpu_px = ctx.composite(&doc)?.to_srgb8_rgba();
        println!("adapter: {}", ctx.adapter_name());
        println!("path: {}", if linear { "linear(F32)" } else { "gamma(U8)" });
        println!("max-abs(CPU,GPU) = {}", max_abs(&cpu, &gpu_px));
        return Ok(());
    }

    let rgba = if gpu {
        let ctx = dcli_gpu::GpuContext::new_headless()?;
        eprintln!("GPU adapter: {}", ctx.adapter_name());
        ctx.composite(&doc)?.to_srgb8_rgba()
    } else {
        dcli_raster::composite(&doc).to_srgb8_rgba()
    };

    match out {
        Some(p) => {
            write_png(&p, w, h, &rgba)?;
            eprintln!("wrote {p} ({}x{}, {} path)", w, h, if gpu { "gpu" } else { "cpu" });
        }
        None => eprintln!("(--out 미지정: PNG 미출력. --diff 또는 --out 사용)"),
    }
    Ok(())
}
