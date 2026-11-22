// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::render::canvas::{Canvas, DrawCmd, LOGICAL_H, LOGICAL_W};
use crate::render::gpu_pipelines::GpuPipelines;
use crate::render::gpu_types::{
    PipeMaskInstance, ShapeInstance, SpriteInstance, StageUniform, TexId, build_color_matrix_table,
};
use crate::render::tex_bank::TexBank;
use std::ops::Range;
use std::sync::{Arc, Mutex};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Adapter, BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource, Buffer,
    BufferDescriptor, BufferUsages, Color, CompositeAlphaMode, Device, DeviceDescriptor, Extent3d,
    FilterMode, Instance, LoadOp, Maintain, MapMode, Operations, PowerPreference, PresentMode,
    Queue, RenderPassColorAttachment, RenderPassDescriptor, RequestAdapterOptions, Sampler,
    SamplerDescriptor, StoreOp, Surface, TexelCopyBufferInfo, TexelCopyBufferLayout,
    TexelCopyTextureInfo, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages, TextureView, TextureViewDescriptor,
};

#[derive(Default)]
struct DynBuf {
    buf: Option<Buffer>,
    cap: u64,
}

impl DynBuf {
    fn upload(&mut self, device: &Device, queue: &Queue, label: &str, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let need = bytes.len() as u64;
        if self.cap < need {
            let cap = need.max(4096).next_power_of_two();
            self.buf = Some(device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size: cap,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.cap = cap;
        }
        if let Some(b) = &self.buf {
            queue.write_buffer(b, 0, bytes);
        }
    }
}

#[derive(Default)]
struct FrameBufs {
    sprite: DynBuf,
    shape: DynBuf,
    pipe: DynBuf,
}

pub struct GpuCore {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub pipelines: GpuPipelines,
    pub surface_format: TextureFormat,
    pub target_format: TextureFormat,
    pub alpha_mode: CompositeAlphaMode,
    pub present_modes: Vec<PresentMode>,
    pub max_tex_dim: u32,
    pub sampler: Sampler,
    pub cmat_buf: Buffer,
    pub stage_cmat_bg: BindGroup,
    pub stage_only_bg: BindGroup,
    bufs: Mutex<FrameBufs>,
}

enum PassOp {
    SpriteTex(TexId, Range<u32>),
    SpriteDirect(Arc<BindGroup>, Range<u32>),
    Shape(Range<u32>),
    PipeMask(TexId, Range<u32>),
    Leak(BindGroup),
}

fn init_from_adapter(
    adapter: &Adapter,
    surface_format: TextureFormat,
    target_format: TextureFormat,
    alpha_mode: CompositeAlphaMode,
    present_modes: Vec<PresentMode>,
) -> Result<GpuCore, String> {
    let limits = adapter.limits();
    let max_tex_dim = limits.max_texture_dimension_2d;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &DeviceDescriptor {
            label: Some("aqualux_device"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            memory_hints: Default::default(),
        },
        None,
    ))
    .map_err(|e| format!("GPU device creation failed: {}", e))?;

    let pipelines = GpuPipelines::new(&device, target_format);
    let sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("linear_clamp"),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..Default::default()
    });
    let stage_buf = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("stage_uni"),
        contents: bytemuck::bytes_of(&StageUniform::for_size(LOGICAL_W as f32, LOGICAL_H as f32)),
        usage: BufferUsages::UNIFORM,
    });
    let cmat_buf = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("cmat_uni"),
        contents: bytemuck::bytes_of(&build_color_matrix_table()),
        usage: BufferUsages::UNIFORM,
    });
    let stage_cmat_bg = device.create_bind_group(&BindGroupDescriptor {
        label: Some("stage_cmat_bg"),
        layout: &pipelines.stage_cmat_bgl,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: stage_buf.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: cmat_buf.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::Sampler(&sampler),
            },
        ],
    });
    let stage_only_bg = device.create_bind_group(&BindGroupDescriptor {
        label: Some("stage_only_bg"),
        layout: &pipelines.stage_only_bgl,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: stage_buf.as_entire_binding(),
        }],
    });

    Ok(GpuCore {
        device: Arc::new(device),
        queue: Arc::new(queue),
        pipelines,
        surface_format,
        target_format,
        alpha_mode,
        present_modes,
        max_tex_dim,
        sampler,
        cmat_buf,
        stage_cmat_bg,
        stage_only_bg,
        bufs: Mutex::new(FrameBufs::default()),
    })
}

fn pick_adapter(instance: &Instance, compatible_surface: Option<&Surface<'_>>) -> Option<Adapter> {
    let req = |power_preference, force_fallback_adapter| {
        pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference,
            compatible_surface,
            force_fallback_adapter,
        }))
    };
    req(PowerPreference::HighPerformance, false).or_else(|| req(PowerPreference::LowPower, true))
}

impl GpuCore {
    pub fn new_for_surface(instance: &Instance, surface: &Surface<'_>) -> Result<Self, String> {
        let adapter = pick_adapter(instance, Some(surface))
            .ok_or_else(|| "No GPU adapter found for window surface".to_string())?;
        let caps = surface.get_capabilities(&adapter);
        let (surface_format, target_format) =
            if let Some(unorm) = caps.formats.iter().copied().find(|f| !f.is_srgb()) {
                (unorm, unorm)
            } else {
                let srgb = caps
                    .formats
                    .first()
                    .copied()
                    .ok_or("No surface formats supported")?;
                (srgb, srgb.remove_srgb_suffix())
            };
        let alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(CompositeAlphaMode::Auto);
        init_from_adapter(
            &adapter,
            surface_format,
            target_format,
            alpha_mode,
            caps.present_modes,
        )
    }

    pub fn new_headless() -> Result<Self, String> {
        let instance = Instance::default();
        let adapter = pick_adapter(&instance, None)
            .ok_or_else(|| "No headless GPU adapter available".to_string())?;
        init_from_adapter(
            &adapter,
            TextureFormat::Rgba8Unorm,
            TextureFormat::Rgba8Unorm,
            CompositeAlphaMode::Auto,
            Vec::new(),
        )
    }

    pub fn render_frame(
        &self,
        canvas: &Canvas,
        bank: &TexBank,
        target_view: &TextureView,
        phys_w: u32,
        phys_h: u32,
    ) {
        let mut sprites: Vec<SpriteInstance> = Vec::with_capacity(256);
        let mut shapes: Vec<ShapeInstance> = Vec::with_capacity(256);
        let mut pipes: Vec<PipeMaskInstance> = Vec::with_capacity(64);
        let mut ops: Vec<PassOp> = Vec::with_capacity(canvas.cmds.len());

        let push_shape =
            |inst: ShapeInstance, shapes: &mut Vec<ShapeInstance>, ops: &mut Vec<PassOp>| {
                let s = shapes.len() as u32;
                shapes.push(inst);
                if let Some(PassOp::Shape(rng)) = ops.last_mut() {
                    rng.end = s + 1;
                } else {
                    ops.push(PassOp::Shape(s..s + 1));
                }
            };

        for cmd in &canvas.cmds {
            match cmd {
                DrawCmd::Clear(r, g, b) => push_shape(
                    ShapeInstance {
                        bounds: [0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32],
                        p0: [0.0; 4],
                        p1: [0.0; 4],
                        fill: [*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0, 1.0],
                        stroke: [0.0; 4],
                        meta: [0.0; 4],
                    },
                    &mut shapes,
                    &mut ops,
                ),
                DrawCmd::SpriteTex(tid, inst) => {
                    let s = sprites.len() as u32;
                    sprites.push(*inst);
                    if let Some(PassOp::SpriteTex(prev, rng)) = ops.last_mut()
                        && *prev == *tid
                    {
                        rng.end = s + 1;
                    } else {
                        ops.push(PassOp::SpriteTex(*tid, s..s + 1));
                    }
                }
                DrawCmd::SpriteDirect(bg, inst) => {
                    let s = sprites.len() as u32;
                    sprites.push(*inst);
                    if let Some(PassOp::SpriteDirect(prev, rng)) = ops.last_mut()
                        && Arc::ptr_eq(prev, bg)
                    {
                        rng.end = s + 1;
                    } else {
                        ops.push(PassOp::SpriteDirect(bg.clone(), s..s + 1));
                    }
                }
                DrawCmd::Shape(inst) => push_shape(*inst, &mut shapes, &mut ops),
                DrawCmd::PipeMask(tid, inst) => {
                    let s = pipes.len() as u32;
                    pipes.push(*inst);
                    if let Some(PassOp::PipeMask(prev, rng)) = ops.last_mut()
                        && *prev == *tid
                    {
                        rng.end = s + 1;
                    } else {
                        ops.push(PassOp::PipeMask(*tid, s..s + 1));
                    }
                }
                DrawCmd::Leak(uni) => {
                    let ub = self.device.create_buffer_init(&BufferInitDescriptor {
                        label: Some("lk_u"),
                        contents: bytemuck::bytes_of(uni.as_ref()),
                        usage: BufferUsages::UNIFORM,
                    });
                    let bg = self.device.create_bind_group(&BindGroupDescriptor {
                        label: Some("lk_bg"),
                        layout: &self.pipelines.leak_bgl,
                        entries: &[BindGroupEntry {
                            binding: 0,
                            resource: ub.as_entire_binding(),
                        }],
                    });
                    ops.push(PassOp::Leak(bg));
                }
            }
        }

        let mut fbs = self.bufs.lock().expect("gpu bufs lock");
        fbs.sprite.upload(
            &self.device,
            &self.queue,
            "f_sp",
            bytemuck::cast_slice(&sprites),
        );
        fbs.shape.upload(
            &self.device,
            &self.queue,
            "f_sh",
            bytemuck::cast_slice(&shapes),
        );
        fbs.pipe.upload(
            &self.device,
            &self.queue,
            "f_pm",
            bytemuck::cast_slice(&pipes),
        );

        let (wf, hf) = (phys_w.max(1) as f32, phys_h.max(1) as f32);
        let scale = (wf / LOGICAL_W as f32).min(hf / LOGICAL_H as f32);
        let (dw, dh) = (
            (LOGICAL_W as f32 * scale).round().max(1.0).min(wf),
            (LOGICAL_H as f32 * scale).round().max(1.0).min(hf),
        );
        let (ox, oy) = (
            ((wf - dw) * 0.5).floor().max(0.0),
            ((hf - dh) * 0.5).floor().max(0.0),
        );

        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut rp = enc.begin_render_pass(&RenderPassDescriptor {
                label: Some("main_frame"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_viewport(ox, oy, dw, dh, 0.0, 1.0);
            rp.set_scissor_rect(ox as u32, oy as u32, dw as u32, dh as u32);
            let texs = bank.textures_ref();
            for op in ops {
                match op {
                    PassOp::SpriteTex(tid, rng) => {
                        if let Some(bg) = texs
                            .get(tid.0 as usize)
                            .and_then(|o| o.as_ref())
                            .map(|t| &t.bind_group)
                            && let Some(vb) = &fbs.sprite.buf
                        {
                            rp.set_pipeline(&self.pipelines.sprite_target);
                            rp.set_bind_group(0, &self.stage_cmat_bg, &[]);
                            rp.set_bind_group(1, bg, &[]);
                            rp.set_vertex_buffer(0, vb.slice(..));
                            rp.draw(0..6, rng);
                        }
                    }
                    PassOp::SpriteDirect(bg, rng) => {
                        if let Some(vb) = &fbs.sprite.buf {
                            rp.set_pipeline(&self.pipelines.sprite_target);
                            rp.set_bind_group(0, &self.stage_cmat_bg, &[]);
                            rp.set_bind_group(1, bg.as_ref(), &[]);
                            rp.set_vertex_buffer(0, vb.slice(..));
                            rp.draw(0..6, rng);
                        }
                    }
                    PassOp::Shape(rng) => {
                        if let Some(vb) = &fbs.shape.buf {
                            rp.set_pipeline(&self.pipelines.shape_target);
                            rp.set_bind_group(0, &self.stage_only_bg, &[]);
                            rp.set_vertex_buffer(0, vb.slice(..));
                            rp.draw(0..6, rng);
                        }
                    }
                    PassOp::PipeMask(tid, rng) => {
                        if let Some(bg) = texs
                            .get(tid.0 as usize)
                            .and_then(|o| o.as_ref())
                            .map(|t| &t.bind_group)
                            && let Some(vb) = &fbs.pipe.buf
                        {
                            rp.set_pipeline(&self.pipelines.pipe_mask);
                            rp.set_bind_group(0, &self.stage_cmat_bg, &[]);
                            rp.set_bind_group(1, bg, &[]);
                            rp.set_vertex_buffer(0, vb.slice(..));
                            rp.draw(0..6, rng);
                        }
                    }
                    PassOp::Leak(leak_bg) => {
                        rp.set_pipeline(&self.pipelines.leak);
                        rp.set_bind_group(0, &self.stage_only_bg, &[]);
                        rp.set_bind_group(1, &leak_bg, &[]);
                        rp.draw(0..6, 0..1);
                    }
                }
            }
        }
        self.queue.submit(Some(enc.finish()));
    }

    pub fn render_to_rgba(&self, canvas: &Canvas, bank: &TexBank, w: u32, h: u32) -> Vec<u8> {
        let (w, h) = (w.max(1), h.max(1));
        let tex = self.device.create_texture(&TextureDescriptor {
            label: Some("off_rb"),
            size: Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: self.target_format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&TextureViewDescriptor::default());
        self.render_frame(canvas, bank, &view, w, h);

        let unpadded = w * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let buf = self.device.create_buffer(&BufferDescriptor {
            label: Some("rb_buf"),
            size: (padded * h) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &buf,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(enc.finish()));

        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(Maintain::Wait);
        let _ = rx.recv();

        let data = slice.get_mapped_range();
        let mut out = vec![0u8; (unpadded * h) as usize];
        for y in 0..h as usize {
            let (s0, d0) = (y * padded as usize, y * unpadded as usize);
            out[d0..d0 + unpadded as usize].copy_from_slice(&data[s0..s0 + unpadded as usize]);
        }
        drop(data);
        buf.unmap();
        if self.target_format == TextureFormat::Bgra8Unorm {
            for px in out.as_chunks_mut::<4>().0 {
                px.swap(0, 2);
            }
        }
        out
    }
}
