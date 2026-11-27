// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::render::canvas::{Canvas, DrawCmd, LOGICAL_H, LOGICAL_W, Rect};
use crate::render::gpu_pipelines::GpuPipelines;
use crate::render::gpu_types::{
    BloomUniform, PipeMaskInstance, PresentUniform, ShapeInstance, SpriteInstance, StageUniform,
    TexId, build_color_matrix_table,
};
use crate::render::hdr::{HdrDisplayCaps, HdrOutputMode, HdrPresentParams, query_surface_hdr_caps};
use crate::render::tex_bank::TexBank;
use std::ops::Range;
use std::sync::{Arc, Mutex};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Adapter, BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource, Buffer,
    BufferDescriptor, BufferUsages, Color, CommandEncoder, CompositeAlphaMode, Device,
    DeviceDescriptor, Extent3d, FilterMode, Instance, InstanceDescriptor, LoadOp, MapMode,
    Operations, PollType, PowerPreference, PresentMode, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, RequestAdapterOptions, Sampler, SamplerDescriptor, StoreOp, Surface,
    TexelCopyBufferInfo, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor,
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

struct HdrSceneTarget {
    width: u32,
    height: u32,
    bloom_w: u32,
    bloom_h: u32,
    _tex: wgpu::Texture,
    view: TextureView,
    _bloom_a: wgpu::Texture,
    bloom_a_view: TextureView,
    _bloom_b: wgpu::Texture,
    bloom_b_view: TextureView,
    bloom_ext_bg: BindGroup,
    bloom_h_bg: BindGroup,
    bloom_v_bg: BindGroup,
    present_bg: BindGroup,
}

#[derive(Default)]
struct FrameBufs {
    sprite: DynBuf,
    shape: DynBuf,
    pipe: DynBuf,
    hdr_scene: Option<HdrSceneTarget>,
}

pub struct GpuCore {
    adapter: Adapter,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub pipelines: GpuPipelines,
    pub surface_format: TextureFormat,
    pub target_format: TextureFormat,
    pub alpha_mode: CompositeAlphaMode,
    pub present_modes: Vec<PresentMode>,
    pub max_tex_dim: u32,
    hdr_caps: HdrDisplayCaps,
    pub sampler: Sampler,
    pub cmat_buf: Buffer,
    pub stage_cmat_bg: BindGroup,
    pub stage_only_bg: BindGroup,
    pub stage_cmat_hdr_bg: BindGroup,
    pub stage_only_hdr_bg: BindGroup,
    present_uni_buf: Buffer,
    bloom_ext_uni: Buffer,
    bloom_h_uni: Buffer,
    bloom_v_uni: Buffer,
    bufs: Mutex<FrameBufs>,
}

enum PassOp {
    SetClip(Option<Rect>),
    SpriteTex(TexId, Range<u32>),
    SpriteDirect(Arc<BindGroup>, Range<u32>),
    Shape(Range<u32>),
    PipeMask(TexId, Range<u32>),
    Leak(BindGroup),
}

fn init_from_adapter(
    adapter: Adapter,
    surface_format: TextureFormat,
    target_format: TextureFormat,
    alpha_mode: CompositeAlphaMode,
    present_modes: Vec<PresentMode>,
    hdr_caps: HdrDisplayCaps,
) -> Result<GpuCore, String> {
    let limits = adapter.limits();
    let max_tex_dim = limits.max_texture_dimension_2d;
    let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
        label: Some("aqualux_device"),
        required_features: wgpu::Features::empty(),
        required_limits: limits,
        experimental_features: Default::default(),
        memory_hints: Default::default(),
        trace: Default::default(),
    }))
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
    let stage_hdr_buf = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("stage_hdr_uni"),
        contents: bytemuck::bytes_of(&StageUniform::for_size_hdr(
            LOGICAL_W as f32,
            LOGICAL_H as f32,
        )),
        usage: BufferUsages::UNIFORM,
    });
    let cmat_buf = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("cmat_uni"),
        contents: bytemuck::bytes_of(&build_color_matrix_table()),
        usage: BufferUsages::UNIFORM,
    });
    let present_uni_buf = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("present_uni"),
        contents: bytemuck::bytes_of(&PresentUniform {
            params: [1.0, 203.0, 1.0, 0.0],
        }),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });
    let mk_bloom_uni = |label| {
        device.create_buffer_init(&BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::bytes_of(&BloomUniform { params: [0.0; 4] }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        })
    };
    let bloom_ext_uni = mk_bloom_uni("bloom_ext_uni");
    let bloom_h_uni = mk_bloom_uni("bloom_h_uni");
    let bloom_v_uni = mk_bloom_uni("bloom_v_uni");
    let make_stage_cmat_bg = |label, buf: &Buffer| {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some(label),
            layout: &pipelines.stage_cmat_bgl,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
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
        })
    };
    let make_stage_only_bg = |label, buf: &Buffer| {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some(label),
            layout: &pipelines.stage_only_bgl,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: buf.as_entire_binding(),
            }],
        })
    };
    let stage_cmat_bg = make_stage_cmat_bg("stage_cmat_bg", &stage_buf);
    let stage_only_bg = make_stage_only_bg("stage_only_bg", &stage_buf);
    let stage_cmat_hdr_bg = make_stage_cmat_bg("stage_cmat_hdr_bg", &stage_hdr_buf);
    let stage_only_hdr_bg = make_stage_only_bg("stage_only_hdr_bg", &stage_hdr_buf);

    Ok(GpuCore {
        adapter,
        device: Arc::new(device),
        queue: Arc::new(queue),
        pipelines,
        surface_format,
        target_format,
        alpha_mode,
        present_modes,
        max_tex_dim,
        hdr_caps,
        sampler,
        cmat_buf,
        stage_cmat_bg,
        stage_only_bg,
        stage_cmat_hdr_bg,
        stage_only_hdr_bg,
        present_uni_buf,
        bloom_ext_uni,
        bloom_h_uni,
        bloom_v_uni,
        bufs: Mutex::new(FrameBufs::default()),
    })
}

fn pick_adapter(instance: &Instance, compatible_surface: Option<&Surface<'_>>) -> Option<Adapter> {
    let req = |power_preference, force_fallback_adapter| {
        pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference,
            compatible_surface,
            force_fallback_adapter,
            apply_limit_buckets: false,
        }))
    };
    req(PowerPreference::HighPerformance, false)
        .or_else(|_| req(PowerPreference::LowPower, true))
        .ok()
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
        let present_modes = caps.present_modes;
        let hdr_caps = query_surface_hdr_caps(&adapter, surface);
        init_from_adapter(
            adapter,
            surface_format,
            target_format,
            alpha_mode,
            present_modes,
            hdr_caps,
        )
    }

    pub fn new_headless() -> Result<Self, String> {
        let instance = Instance::new(InstanceDescriptor::new_without_display_handle());
        let adapter = pick_adapter(&instance, None)
            .ok_or_else(|| "No headless GPU adapter available".to_string())?;
        init_from_adapter(
            adapter,
            TextureFormat::Rgba8Unorm,
            TextureFormat::Rgba8Unorm,
            CompositeAlphaMode::Auto,
            Vec::new(),
            HdrDisplayCaps::unsupported(),
        )
    }

    pub fn hdr_caps(&self) -> HdrDisplayCaps {
        self.hdr_caps
    }

    pub fn query_surface_hdr(&self, surface: &Surface<'_>) -> HdrDisplayCaps {
        query_surface_hdr_caps(&self.adapter, surface)
    }

    fn prepare_scene_ops(&self, canvas: &Canvas, fbs: &mut FrameBufs) -> Vec<PassOp> {
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
                DrawCmd::SetClip(rect) => ops.push(PassOp::SetClip(*rect)),
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
        ops
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_scene_pass(
        &self,
        enc: &mut CommandEncoder,
        fbs: &FrameBufs,
        ops: Vec<PassOp>,
        bank: &TexBank,
        target_view: &TextureView,
        phys_w: u32,
        phys_h: u32,
        hdr: bool,
    ) {
        let (sp_pipe, sh_pipe, pm_pipe, lk_pipe, sc_bg, so_bg) = if hdr {
            (
                &self.pipelines.sprite_hdr,
                &self.pipelines.shape_hdr,
                &self.pipelines.pipe_mask_hdr,
                &self.pipelines.leak_hdr,
                &self.stage_cmat_hdr_bg,
                &self.stage_only_hdr_bg,
            )
        } else {
            (
                &self.pipelines.sprite_target,
                &self.pipelines.shape_target,
                &self.pipelines.pipe_mask,
                &self.pipelines.leak,
                &self.stage_cmat_bg,
                &self.stage_only_bg,
            )
        };

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

        let mut rp = enc.begin_render_pass(&RenderPassDescriptor {
            label: Some(if hdr { "hdr_scene" } else { "main_frame" }),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: target_view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_viewport(ox, oy, dw, dh, 0.0, 1.0);
        let (vp_x, vp_y, vp_w, vp_h) = (ox as u32, oy as u32, dw as u32, dh as u32);
        rp.set_scissor_rect(vp_x, vp_y, vp_w, vp_h);
        let mut clip_empty = false;
        let texs = bank.textures_ref();
        for op in ops {
            match op {
                PassOp::SetClip(None) => {
                    clip_empty = false;
                    rp.set_scissor_rect(vp_x, vp_y, vp_w, vp_h);
                }
                PassOp::SetClip(Some(r)) => {
                    let x0 = (ox + r.x * scale).floor().clamp(ox, ox + dw);
                    let y0 = (oy + r.y * scale).floor().clamp(oy, oy + dh);
                    let x1 = (ox + (r.x + r.w) * scale).ceil().clamp(x0, ox + dw);
                    let y1 = (oy + (r.y + r.h) * scale).ceil().clamp(y0, oy + dh);
                    let sx = (x0 as u32).min(phys_w);
                    let sy = (y0 as u32).min(phys_h);
                    let sw = ((x1 - x0) as u32).min(phys_w - sx);
                    let sh = ((y1 - y0) as u32).min(phys_h - sy);
                    if sw == 0 || sh == 0 {
                        clip_empty = true;
                    } else {
                        clip_empty = false;
                        rp.set_scissor_rect(sx, sy, sw, sh);
                    }
                }
                PassOp::SpriteTex(tid, rng) => {
                    if !clip_empty
                        && let Some(bg) = texs
                            .get(tid.0 as usize)
                            .and_then(|o| o.as_ref())
                            .map(|t| &t.bind_group)
                        && let Some(vb) = &fbs.sprite.buf
                    {
                        rp.set_pipeline(sp_pipe);
                        rp.set_bind_group(0, sc_bg, &[]);
                        rp.set_bind_group(1, bg, &[]);
                        rp.set_vertex_buffer(0, vb.slice(..));
                        rp.draw(0..6, rng);
                    }
                }
                PassOp::SpriteDirect(bg, rng) => {
                    if !clip_empty && let Some(vb) = &fbs.sprite.buf {
                        rp.set_pipeline(sp_pipe);
                        rp.set_bind_group(0, sc_bg, &[]);
                        rp.set_bind_group(1, bg.as_ref(), &[]);
                        rp.set_vertex_buffer(0, vb.slice(..));
                        rp.draw(0..6, rng);
                    }
                }
                PassOp::Shape(rng) => {
                    if !clip_empty && let Some(vb) = &fbs.shape.buf {
                        rp.set_pipeline(sh_pipe);
                        rp.set_bind_group(0, so_bg, &[]);
                        rp.set_vertex_buffer(0, vb.slice(..));
                        rp.draw(0..6, rng);
                    }
                }
                PassOp::PipeMask(tid, rng) => {
                    if !clip_empty
                        && let Some(bg) = texs
                            .get(tid.0 as usize)
                            .and_then(|o| o.as_ref())
                            .map(|t| &t.bind_group)
                        && let Some(vb) = &fbs.pipe.buf
                    {
                        rp.set_pipeline(pm_pipe);
                        rp.set_bind_group(0, sc_bg, &[]);
                        rp.set_bind_group(1, bg, &[]);
                        rp.set_vertex_buffer(0, vb.slice(..));
                        rp.draw(0..6, rng);
                    }
                }
                PassOp::Leak(leak_bg) => {
                    if !clip_empty {
                        rp.set_pipeline(lk_pipe);
                        rp.set_bind_group(0, so_bg, &[]);
                        rp.set_bind_group(1, &leak_bg, &[]);
                        rp.draw(0..6, 0..1);
                    }
                }
            }
        }
    }

    pub fn render_frame(
        &self,
        canvas: &Canvas,
        bank: &TexBank,
        target_view: &TextureView,
        phys_w: u32,
        phys_h: u32,
    ) {
        let mut fbs = self.bufs.lock().expect("gpu bufs lock");
        let ops = self.prepare_scene_ops(canvas, &mut fbs);
        let mut enc = self.device.create_command_encoder(&Default::default());
        self.encode_scene_pass(
            &mut enc,
            &fbs,
            ops,
            bank,
            target_view,
            phys_w,
            phys_h,
            false,
        );
        self.queue.submit(Some(enc.finish()));
    }

    pub fn render_frame_hdr(
        &self,
        canvas: &Canvas,
        bank: &TexBank,
        swapchain_view: &TextureView,
        phys_w: u32,
        phys_h: u32,
        params: HdrPresentParams,
    ) {
        let (w, h) = (phys_w.max(1), phys_h.max(1));
        let mut fbs = self.bufs.lock().expect("gpu bufs lock");
        let need_new = fbs
            .hdr_scene
            .as_ref()
            .is_none_or(|t| t.width != w || t.height != h);
        if need_new {
            let mk_hdr_tex = |label, tw, th| {
                self.device.create_texture(&TextureDescriptor {
                    label: Some(label),
                    size: Extent3d {
                        width: tw,
                        height: th,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Rgba16Float,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
            };
            let (bw, bh) = ((w / 2).max(1), (h / 2).max(1));
            let tex = mk_hdr_tex("hdr_scene_tex", w, h);
            let view = tex.create_view(&TextureViewDescriptor::default());
            let bloom_a = mk_hdr_tex("hdr_bloom_a", bw, bh);
            let bloom_a_view = bloom_a.create_view(&TextureViewDescriptor::default());
            let bloom_b = mk_hdr_tex("hdr_bloom_b", bw, bh);
            let bloom_b_view = bloom_b.create_view(&TextureViewDescriptor::default());

            let mk_bloom_bg = |label, uni: &Buffer, src: &TextureView| {
                self.device.create_bind_group(&BindGroupDescriptor {
                    label: Some(label),
                    layout: &self.pipelines.bloom_bgl,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: uni.as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::TextureView(src),
                        },
                        BindGroupEntry {
                            binding: 2,
                            resource: BindingResource::Sampler(&self.sampler),
                        },
                    ],
                })
            };
            let bloom_ext_bg = mk_bloom_bg("bloom_ext_bg", &self.bloom_ext_uni, &view);
            let bloom_h_bg = mk_bloom_bg("bloom_h_bg", &self.bloom_h_uni, &bloom_a_view);
            let bloom_v_bg = mk_bloom_bg("bloom_v_bg", &self.bloom_v_uni, &bloom_b_view);
            let present_bg = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("hdr_present_bg"),
                layout: &self.pipelines.present_bgl,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: self.present_uni_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&view),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::TextureView(&bloom_a_view),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.queue.write_buffer(
                &self.bloom_ext_uni,
                0,
                bytemuck::bytes_of(&BloomUniform {
                    params: [1.0 / w as f32, 1.0 / h as f32, 0.0, 0.0],
                }),
            );
            self.queue.write_buffer(
                &self.bloom_h_uni,
                0,
                bytemuck::bytes_of(&BloomUniform {
                    params: [1.0 / bw as f32, 1.0 / bh as f32, 1.0, 0.0],
                }),
            );
            self.queue.write_buffer(
                &self.bloom_v_uni,
                0,
                bytemuck::bytes_of(&BloomUniform {
                    params: [1.0 / bw as f32, 1.0 / bh as f32, 0.0, 1.0],
                }),
            );
            fbs.hdr_scene = Some(HdrSceneTarget {
                width: w,
                height: h,
                bloom_w: bw,
                bloom_h: bh,
                _tex: tex,
                view,
                _bloom_a: bloom_a,
                bloom_a_view,
                _bloom_b: bloom_b,
                bloom_b_view,
                bloom_ext_bg,
                bloom_h_bg,
                bloom_v_bg,
                present_bg,
            });
        }

        let mode_f = match params.mode {
            HdrOutputMode::ScRgb => 0.0,
            HdrOutputMode::Hdr10Pq => 1.0,
        };
        let uni = PresentUniform {
            params: [
                params.scrgb_white_scale,
                params.sdr_white_nits,
                params.peak_headroom,
                mode_f,
            ],
        };
        self.queue
            .write_buffer(&self.present_uni_buf, 0, bytemuck::bytes_of(&uni));

        let ops = self.prepare_scene_ops(canvas, &mut fbs);
        let mut enc = self.device.create_command_encoder(&Default::default());
        let scene = fbs.hdr_scene.as_ref().expect("hdr scene target");
        self.encode_scene_pass(&mut enc, &fbs, ops, bank, &scene.view, w, h, true);
        let run_full = |enc: &mut CommandEncoder, label, pipe, bg, view, tw: u32, th: u32| {
            let mut rp = enc.begin_render_pass(&RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_viewport(0.0, 0.0, tw as f32, th as f32, 0.0, 1.0);
            rp.set_scissor_rect(0, 0, tw, th);
            rp.set_pipeline(pipe);
            rp.set_bind_group(0, bg, &[]);
            rp.draw(0..3, 0..1);
        };
        run_full(
            &mut enc,
            "hdr_bloom_ext",
            &self.pipelines.bloom_extract,
            &scene.bloom_ext_bg,
            &scene.bloom_a_view,
            scene.bloom_w,
            scene.bloom_h,
        );
        run_full(
            &mut enc,
            "hdr_bloom_h",
            &self.pipelines.bloom_blur,
            &scene.bloom_h_bg,
            &scene.bloom_b_view,
            scene.bloom_w,
            scene.bloom_h,
        );
        run_full(
            &mut enc,
            "hdr_bloom_v",
            &self.pipelines.bloom_blur,
            &scene.bloom_v_bg,
            &scene.bloom_a_view,
            scene.bloom_w,
            scene.bloom_h,
        );
        let present_pipe = match params.mode {
            HdrOutputMode::ScRgb => &self.pipelines.present_scrgb,
            HdrOutputMode::Hdr10Pq => &self.pipelines.present_pq,
        };
        run_full(
            &mut enc,
            "hdr_present",
            present_pipe,
            &scene.present_bg,
            swapchain_view,
            w,
            h,
        );
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
        let _ = self.device.poll(PollType::wait_indefinitely());
        let _ = rx.recv();

        let data = slice.get_mapped_range().expect("mapped buffer range");
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

    #[cfg(test)]
    fn readback_texture_bytes(
        &self,
        format: TextureFormat,
        bpp: u32,
        w: u32,
        h: u32,
        draw: impl FnOnce(&TextureView),
    ) -> Vec<u8> {
        let (w, h) = (w.max(1), h.max(1));
        let tex = self.device.create_texture(&TextureDescriptor {
            label: Some("hdr_off_rb"),
            size: Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&TextureViewDescriptor::default());
        draw(&view);

        let unpadded = w * bpp;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let buf = self.device.create_buffer(&BufferDescriptor {
            label: Some("hdr_rb_buf"),
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
        let _ = self.device.poll(PollType::wait_indefinitely());
        let _ = rx.recv();

        let data = slice.get_mapped_range().expect("mapped buffer range");
        let mut out = vec![0u8; (unpadded * h) as usize];
        for y in 0..h as usize {
            let (s0, d0) = (y * padded as usize, y * unpadded as usize);
            out[d0..d0 + unpadded as usize].copy_from_slice(&data[s0..s0 + unpadded as usize]);
        }
        drop(data);
        buf.unmap();
        out
    }

    #[cfg(test)]
    pub fn render_to_hdr_f32(
        &self,
        canvas: &Canvas,
        bank: &TexBank,
        w: u32,
        h: u32,
        params: HdrPresentParams,
    ) -> Vec<[f32; 4]> {
        let bytes = self.readback_texture_bytes(TextureFormat::Rgba16Float, 8, w, h, |view| {
            self.render_frame_hdr(canvas, bank, view, w, h, params);
        });
        bytes
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| {
                [
                    f16_to_f32(u16::from_le_bytes([c[0], c[1]])),
                    f16_to_f32(u16::from_le_bytes([c[2], c[3]])),
                    f16_to_f32(u16::from_le_bytes([c[4], c[5]])),
                    f16_to_f32(u16::from_le_bytes([c[6], c[7]])),
                ]
            })
            .collect()
    }

    #[cfg(test)]
    pub fn render_to_hdr10_u10(
        &self,
        canvas: &Canvas,
        bank: &TexBank,
        w: u32,
        h: u32,
        params: HdrPresentParams,
    ) -> Vec<[u16; 4]> {
        let bytes = self.readback_texture_bytes(TextureFormat::Rgb10a2Unorm, 4, w, h, |view| {
            self.render_frame_hdr(canvas, bank, view, w, h, params);
        });
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| {
                let word = u32::from_le_bytes(*c);
                [
                    (word & 0x3FF) as u16,
                    ((word >> 10) & 0x3FF) as u16,
                    ((word >> 20) & 0x3FF) as u16,
                    ((word >> 30) & 0x3) as u16,
                ]
            })
            .collect()
    }
}

#[cfg(test)]
fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits >> 15) as u32) << 31;
    let exp = (bits >> 10) & 0x1F;
    let frac = bits & 0x3FF;
    let f_bits = if exp == 0 {
        if frac == 0 {
            sign
        } else {
            let mut e = 127 - 15 + 1;
            let mut m = frac as u32;
            while (m & 0x400) == 0 {
                m <<= 1;
                e -= 1;
            }
            m &= 0x3FF;
            sign | (e << 23) | (m << 13)
        }
    } else if exp == 31 {
        sign | (0xFF << 23) | ((frac as u32) << 13)
    } else {
        let e = (exp as u32) + (127 - 15);
        sign | (e << 23) | ((frac as u32) << 13)
    };
    f32::from_bits(f_bits)
}
