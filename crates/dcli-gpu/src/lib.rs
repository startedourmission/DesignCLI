//! GPU 가속 프리뷰 + 윈도리스 export 경로 (wgpu).
//!
//! 핵심 가설(Phase 0): surface 없이 wgpu로 합성 → 텍스처 → buffer readback →
//! 픽셀을 CPU 정본과 비교. macOS=Metal, Linux=Vulkan, Windows=DX12, 그리고 헤드리스.
//!
//! 셰이딩 수학은 `shaders/blend.wgsl`이 `dcli-raster`와 1:1로 복제하며,
//! 결과는 CPU 정본 대비 SSIM/max-abs ~1e-4 게이트로 검증된다(parity 테스트).

#![forbid(unsafe_code)]

use bytemuck::{Pod, Zeroable};
use dcli_color::LinearPremul;
use dcli_model::{BlendMode, Document};
use dcli_tile::Surface;
use std::borrow::Cow;

const MAX_LAYERS: usize = 8;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LayerMeta {
    blend: u32,
    opacity: f32,
    _pad0: f32,
    _pad1: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    layer_count: u32,
    blend_space: u32,
    _pad0: u32,
    _pad1: u32,
    layers: [LayerMeta; MAX_LAYERS],
}

fn blend_code(b: BlendMode) -> u32 {
    match b {
        BlendMode::Normal => 0,
        BlendMode::Multiply => 1,
        BlendMode::Screen => 2,
    }
}

/// 윈도리스 GPU 컨텍스트.
pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_name: String,
}

impl GpuContext {
    /// 헤드리스(surface 없는) GPU 컨텍스트를 생성한다.
    pub fn new_headless() -> anyhow::Result<Self> {
        pollster::block_on(Self::new_headless_async())
    }

    async fn new_headless_async() -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("적합한 GPU adapter를 찾지 못함"))?;
        let adapter_name = adapter.get_info().name.clone();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("dcli-gpu device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await?;
        Ok(Self { device, queue, adapter_name })
    }

    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    /// 문서를 GPU로 합성하고 결과를 linear-premul 표면으로 반환한다.
    pub fn composite(&self, doc: &Document) -> anyhow::Result<Surface> {
        // bottom-to-top 가시 페인트 노드를 (blend, opacity, surface) 쌍으로 수집.
        // 표면이 스토어에 없거나 픽셀 없는 노드(그룹)는 제외(CPU 정본과 동일 규칙).
        let visible: Vec<(BlendMode, f32, &Surface)> = doc
            .iter_bottom_to_top()
            .filter(|n| n.visible && n.opacity > 0.0)
            .filter_map(|n| {
                let sid = n.surface_id()?;
                let s = doc.pixels().get(sid)?;
                Some((n.blend, n.opacity, s))
            })
            .collect();
        anyhow::ensure!(
            visible.len() <= MAX_LAYERS,
            "Phase 0~1 GPU 경로는 최대 {MAX_LAYERS}개 레이어 지원(요청: {})",
            visible.len()
        );

        let (w, h) = (doc.width, doc.height);
        let device = &self.device;
        let queue = &self.queue;

        // --- 레이어 텍스처 배열 (RGBA32Float, linear-premul 그대로) ---
        let layer_count = visible.len().max(1) as u32;
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("layers"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: layer_count,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        for (i, (_, _, surface)) in visible.iter().enumerate() {
            let data = surface_to_f32(surface);
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: i as u32 },
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&data),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 16), // 4 floats * 4 bytes
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
        }
        let tex_view = tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        // --- uniforms ---
        let mut layers = [LayerMeta { blend: 0, opacity: 1.0, _pad0: 0.0, _pad1: 0.0 }; MAX_LAYERS];
        for (i, (blend, opacity, _)) in visible.iter().enumerate() {
            layers[i] = LayerMeta {
                blend: blend_code(*blend),
                opacity: *opacity,
                _pad0: 0.0,
                _pad1: 0.0,
            };
        }
        let uniforms = Uniforms {
            layer_count: visible.len() as u32,
            blend_space: match doc.blend_space {
                dcli_color::BlendSpace::Gamma => 0,
                dcli_color::BlendSpace::Linear => 1,
            },
            _pad0: 0,
            _pad1: 0,
            layers,
        };
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&ubuf, 0, bytemuck::bytes_of(&uniforms));

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("samp"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // --- 파이프라인 ---
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blend"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/blend.wgsl"))),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&tex_view) },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pl"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipe"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba32Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // --- 렌더 타깃 (RGBA32Float, COPY_SRC) ---
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("target"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target_view = target.create_view(&Default::default());

        // readback buffer: bytes_per_row는 256 정렬 필요.
        let unpadded = w * 16;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("enc"),
        });
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rp"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&pipeline);
            rp.set_bind_group(0, &bind_group, &[]);
            rp.draw(0..3, 0..1);
        }
        enc.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &readback,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit(Some(enc.finish()));

        // --- map & read ---
        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = slice.get_mapped_range();
        let mut out = Surface::new(w, h);
        let px = out.pixels_mut();
        for y in 0..h {
            let row = &data[(y * padded) as usize..];
            let floats: &[f32] = bytemuck::cast_slice(&row[..(w * 16) as usize]);
            for x in 0..w {
                let o = (x * 4) as usize;
                px[(y * w + x) as usize] = LinearPremul {
                    r: floats[o],
                    g: floats[o + 1],
                    b: floats[o + 2],
                    a: floats[o + 3],
                };
            }
        }
        drop(data);
        readback.unmap();
        Ok(out)
    }
}

/// Surface(linear-premul) → RGBA32Float 평면 배열.
fn surface_to_f32(s: &Surface) -> Vec<f32> {
    let mut out = Vec::with_capacity(s.pixels().len() * 4);
    for p in s.pixels() {
        out.extend_from_slice(&[p.r, p.g, p.b, p.a]);
    }
    out
}
