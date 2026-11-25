// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::RgbaImage;
use crate::game::Side;
use crate::render::gpu_core::GpuCore;
use crate::render::gpu_types::{BlurUniform, ShapeInstance, SpriteInstance, StageUniform};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindingResource, BufferUsages, Color, CommandEncoder,
    Extent3d, LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
    TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
};

pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
}

#[inline]
fn premul_u8(c: u8, a: u8) -> u8 {
    ((c as u32 * a as u32 + 127) / 255) as u8
}

pub fn make_gpu_texture(gpu: &GpuCore, w: u32, h: u32, usages: TextureUsages) -> GpuTexture {
    let w = w.max(1);
    let h = h.max(1);
    let texture = gpu.device.create_texture(&TextureDescriptor {
        label: Some("gpu_tex"),
        size: Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: usages,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    let bind_group = gpu.device.create_bind_group(&BindGroupDescriptor {
        label: Some("tex_bg"),
        layout: &gpu.pipelines.tex_bgl,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::TextureView(&view),
        }],
    });
    GpuTexture {
        texture,
        view,
        bind_group,
        width: w,
        height: h,
    }
}

pub fn upload_premul_texture(gpu: &GpuCore, img: &RgbaImage) -> GpuTexture {
    let w = img.width.max(1);
    let h = img.height.max(1);
    let mut premul = vec![0u8; (w * h * 4) as usize];
    for (s, d) in img
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .zip(premul.as_chunks_mut::<4>().0.iter_mut())
    {
        let a = s[3];
        if a == 0 {
            continue;
        } else if a == 255 {
            d.copy_from_slice(s);
        } else {
            d[0] = premul_u8(s[0], a);
            d[1] = premul_u8(s[1], a);
            d[2] = premul_u8(s[2], a);
            d[3] = a;
        }
    }
    let tex = make_gpu_texture(
        gpu,
        w,
        h,
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
    );
    gpu.queue.write_texture(
        TexelCopyTextureInfo {
            texture: &tex.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        &premul,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: Some(h),
        },
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    tex
}

fn run_blur_pass(
    gpu: &GpuCore,
    enc: &mut CommandEncoder,
    src_view: &wgpu::TextureView,
    dst: &GpuTexture,
    uni: BlurUniform,
) {
    let ub = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some("blur_uni"),
        contents: bytemuck::bytes_of(&uni),
        usage: BufferUsages::UNIFORM,
    });
    let bg = gpu.device.create_bind_group(&BindGroupDescriptor {
        label: Some("blur_bg"),
        layout: &gpu.pipelines.blur_bgl,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: ub.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(src_view),
            },
        ],
    });
    let mut rp = enc.begin_render_pass(&RenderPassDescriptor {
        label: Some("blur_pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: &dst.view,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(Color::TRANSPARENT),
                store: StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    rp.set_pipeline(&gpu.pipelines.blur_unorm);
    rp.set_bind_group(0, &bg, &[]);
    rp.draw(0..3, 0..1);
}

#[allow(clippy::too_many_arguments)]
pub fn encode_blur_into(
    gpu: &GpuCore,
    enc: &mut CommandEncoder,
    src_view: &wgpu::TextureView,
    src_w: u32,
    src_h: u32,
    dist: f32,
    angle_deg: f32,
    color: (u8, u8, u8),
    alpha: f32,
    blur: f32,
    strength: f32,
    pad: u32,
) -> GpuTexture {
    let dw = (src_w + pad * 2).max(1);
    let dh = (src_h + pad * 2).max(1);
    let sigma = (blur * 0.45).max(0.5);
    let rad = angle_deg.to_radians();
    let ox = (dist * rad.cos()).round();
    let oy = (dist * rad.sin()).round();
    let gain = alpha.clamp(0.0, 1.0) * strength.max(0.1);

    let inter = make_gpu_texture(
        gpu,
        dw,
        dh,
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
    );
    let out = make_gpu_texture(
        gpu,
        dw,
        dh,
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
    );
    run_blur_pass(
        gpu,
        enc,
        src_view,
        &inter,
        BlurUniform {
            color: [0.0; 4],
            pass_info: [1.0, 0.0, sigma, 1.0],
            size_info: [src_w as f32, src_h as f32, pad as f32 + ox, pad as f32 + oy],
        },
    );
    run_blur_pass(
        gpu,
        enc,
        &inter.view,
        &out,
        BlurUniform {
            color: [
                color.0 as f32 / 255.0,
                color.1 as f32 / 255.0,
                color.2 as f32 / 255.0,
                1.0,
            ],
            pass_info: [0.0, 1.0, sigma, gain],
            size_info: [dw as f32, dh as f32, 0.0, 0.0],
        },
    );
    out
}

#[allow(clippy::too_many_arguments)]
pub fn bake_blur_texture(
    gpu: &GpuCore,
    src: &GpuTexture,
    dist: f32,
    angle_deg: f32,
    color: (u8, u8, u8),
    alpha: f32,
    blur: f32,
    strength: f32,
    pad: u32,
) -> GpuTexture {
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    let out = encode_blur_into(
        gpu, &mut enc, &src.view, src.width, src.height, dist, angle_deg, color, alpha, blur,
        strength, pad,
    );
    gpu.queue.submit(Some(enc.finish()));
    out
}

pub fn make_stage_cmat_bg(gpu: &GpuCore, w: f32, h: f32) -> (wgpu::BindGroup, wgpu::BindGroup) {
    let ubuf = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some("stage_bake"),
        contents: bytemuck::bytes_of(&StageUniform::for_size(w, h)),
        usage: BufferUsages::UNIFORM,
    });
    let sc_bg = gpu.device.create_bind_group(&BindGroupDescriptor {
        label: Some("sc_bake"),
        layout: &gpu.pipelines.stage_cmat_bgl,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: ubuf.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: gpu.cmat_buf.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::Sampler(&gpu.sampler),
            },
        ],
    });
    let so_bg = gpu.device.create_bind_group(&BindGroupDescriptor {
        label: Some("so_bake"),
        layout: &gpu.pipelines.stage_only_bgl,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: ubuf.as_entire_binding(),
        }],
    });
    (sc_bg, so_bg)
}

fn composite_two_sprites(
    gpu: &GpuCore,
    enc: &mut CommandEncoder,
    size: (u32, u32),
    a: (&wgpu::BindGroup, SpriteInstance),
    b: (&wgpu::BindGroup, SpriteInstance),
) -> GpuTexture {
    let (out_w, out_h) = size;
    let out = make_gpu_texture(
        gpu,
        out_w,
        out_h,
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
    );
    let (sc_bg, _) = make_stage_cmat_bg(gpu, out_w as f32, out_h as f32);
    let vb = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some("comp2_vb"),
        contents: bytemuck::cast_slice(&[a.1, b.1]),
        usage: BufferUsages::VERTEX,
    });
    let mut rp = enc.begin_render_pass(&RenderPassDescriptor {
        label: Some("comp2_pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: &out.view,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(Color::TRANSPARENT),
                store: StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    rp.set_pipeline(&gpu.pipelines.sprite_unorm);
    rp.set_bind_group(0, &sc_bg, &[]);
    rp.set_vertex_buffer(0, vb.slice(..));
    rp.set_bind_group(1, a.0, &[]);
    rp.draw(0..6, 0..1);
    rp.set_bind_group(1, b.0, &[]);
    rp.draw(0..6, 1..2);
    drop(rp);
    out
}

fn full_sprite(w: f32, h: f32, x: f32, y: f32) -> SpriteInstance {
    SpriteInstance {
        row0: [1.0, 0.0, x, w],
        row1: [0.0, 1.0, y, h],
        uv_rect: [0.0, 0.0, 1.0, 1.0],
        params: [0.0, 1.0, 1.0, 0.0],
        tint: [1.0, 1.0, 1.0, 0.0],
    }
}

pub fn bake_cursor_texture(gpu: &GpuCore, src: &GpuTexture, pad: u32) -> GpuTexture {
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    let sh = encode_blur_into(
        gpu,
        &mut enc,
        &src.view,
        src.width,
        src.height,
        4.0,
        45.0,
        (0x17, 0x16, 0x29),
        1.0,
        16.0,
        1.0,
        pad,
    );
    let out = composite_two_sprites(
        gpu,
        &mut enc,
        (sh.width, sh.height),
        (
            &sh.bind_group,
            full_sprite(sh.width as f32, sh.height as f32, 0.0, 0.0),
        ),
        (
            &src.bind_group,
            full_sprite(src.width as f32, src.height as f32, pad as f32, pad as f32),
        ),
    );
    gpu.queue.submit(Some(enc.finish()));
    out
}

pub fn bake_path_highlight_texture(gpu: &GpuCore, entry: Side, exit: Side, pad: u32) -> GpuTexture {
    let size = 128 + pad * 2;
    let off = pad as f32;
    let pt = |s: Side| -> (f32, f32) {
        match s {
            Side::Up => (off + 64.0, off + 19.0),
            Side::Right => (off + 109.0, off + 64.0),
            Side::Down => (off + 64.0, off + 109.0),
            Side::Left => (off + 19.0, off + 64.0),
            Side::Center => (off + 64.0, off + 64.0),
        }
    };
    let (a, c, b) = (pt(entry), pt(Side::Center), pt(exit));
    let cap = |x0: f32, y0: f32, x1: f32, y1: f32| {
        let bx = x0.min(x1) - 8.0;
        let by = y0.min(y1) - 8.0;
        ShapeInstance {
            bounds: [bx, by, x0.max(x1) - bx + 8.0, y0.max(y1) - by + 8.0],
            p0: [x0, y0, x1, y1],
            p1: [0.0; 4],
            fill: [0x33 as f32 / 255.0, 1.0, 0.0, 1.0],
            stroke: [0.0; 4],
            meta: [4.0, 0.0, 6.0, 0.0],
        }
    };
    let caps = if entry != Side::Center {
        vec![cap(a.0, a.1, c.0, c.1), cap(c.0, c.1, b.0, b.1)]
    } else {
        vec![cap(a.0, a.1, b.0, b.1)]
    };

    let core = make_gpu_texture(
        gpu,
        size,
        size,
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
    );
    let (_, so_bg) = make_stage_cmat_bg(gpu, size as f32, size as f32);
    let cvb = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some("ph_vb"),
        contents: bytemuck::cast_slice(&caps),
        usage: BufferUsages::VERTEX,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    {
        let mut rp = enc.begin_render_pass(&RenderPassDescriptor {
            label: Some("ph_core"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &core.view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::TRANSPARENT),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_pipeline(&gpu.pipelines.shape_unorm);
        rp.set_bind_group(0, &so_bg, &[]);
        rp.set_vertex_buffer(0, cvb.slice(..));
        rp.draw(0..6, 0..caps.len() as u32);
    }
    let glow = encode_blur_into(
        gpu,
        &mut enc,
        &core.view,
        size,
        size,
        0.0,
        0.0,
        (0x33, 0xFF, 0x00),
        1.0,
        16.0,
        2.0,
        0,
    );
    let full = full_sprite(size as f32, size as f32, 0.0, 0.0);
    let out = composite_two_sprites(
        gpu,
        &mut enc,
        (size, size),
        (&glow.bind_group, full),
        (&core.bind_group, full),
    );
    gpu.queue.submit(Some(enc.finish()));
    out
}

pub fn bake_rainbow_texture(gpu: &GpuCore) -> GpuTexture {
    let core = make_gpu_texture(
        gpu,
        1600,
        1200,
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
    );
    let (_, so_bg) = make_stage_cmat_bg(gpu, 800.0, 600.0);
    let sh = ShapeInstance {
        bounds: [0.0, 120.0, 800.0, 480.0],
        p0: [0.0; 4],
        p1: [0.0; 4],
        fill: [1.0; 4],
        stroke: [0.0; 4],
        meta: [6.0, 0.0, 0.0, 1.0],
    };
    let vb = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some("rb_bake_vb"),
        contents: bytemuck::bytes_of(&sh),
        usage: BufferUsages::VERTEX,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    {
        let mut rp = enc.begin_render_pass(&RenderPassDescriptor {
            label: Some("rb_bake"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &core.view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::TRANSPARENT),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_pipeline(&gpu.pipelines.shape_unorm);
        rp.set_bind_group(0, &so_bg, &[]);
        rp.set_vertex_buffer(0, vb.slice(..));
        rp.draw(0..6, 0..1);
    }
    let glow = encode_blur_into(
        gpu,
        &mut enc,
        &core.view,
        1600,
        1200,
        0.0,
        0.0,
        (255, 255, 221),
        0.9,
        16.0,
        1.3,
        0,
    );
    let full = full_sprite(1600.0, 1200.0, 0.0, 0.0);
    let out = composite_two_sprites(
        gpu,
        &mut enc,
        (1600, 1200),
        (&glow.bind_group, full),
        (&core.bind_group, full),
    );
    gpu.queue.submit(Some(enc.finish()));
    out
}

/// Bakes a 4x supersampled `FireStar` (`3x3` white square + Flash `GlowFilter(rgb)` blur 6, strength 2).
pub fn bake_fire_star_texture(gpu: &GpuCore, rgb: (u8, u8, u8)) -> GpuTexture {
    const SCALE: u32 = 4;
    const CORE_SZ: u32 = 3 * SCALE;
    const PAD: u32 = 36;
    let core = upload_premul_texture(
        gpu,
        &RgbaImage {
            width: CORE_SZ,
            height: CORE_SZ,
            data: vec![255; (CORE_SZ * CORE_SZ * 4) as usize],
        },
    );
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    let glow = encode_blur_into(
        gpu,
        &mut enc,
        &core.view,
        CORE_SZ,
        CORE_SZ,
        0.0,
        0.0,
        rgb,
        1.0,
        6.0 * SCALE as f32,
        2.0,
        PAD,
    );
    let out = composite_two_sprites(
        gpu,
        &mut enc,
        (glow.width, glow.height),
        (
            &glow.bind_group,
            full_sprite(glow.width as f32, glow.height as f32, 0.0, 0.0),
        ),
        (
            &core.bind_group,
            full_sprite(CORE_SZ as f32, CORE_SZ as f32, PAD as f32, PAD as f32),
        ),
    );
    gpu.queue.submit(Some(enc.finish()));
    out
}
