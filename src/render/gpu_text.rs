// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::{GlyphOp, SwfFont};
use crate::render::gpu_bake::{GpuTexture, encode_blur_into, make_gpu_texture, make_stage_cmat_bg};
use crate::render::gpu_core::GpuCore;
use crate::render::gpu_types::{GlyphUniform, GlyphVertex, ShapeInstance, SpriteInstance};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BufferUsages, Color, Extent3d, LoadOp, Operations,
    RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor, StoreOp,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
};

pub struct BakedGpuText {
    pub texture: GpuTexture,
    pub logical_w: f32,
    pub logical_h: f32,
    pub origin_x: f32,
    pub origin_y: f32,
}

fn push_quad_points(ax: f32, ay: f32, cx: f32, cy: f32, bx: f32, by: f32, out: &mut Vec<[f32; 2]>) {
    const STEPS: usize = 6;
    for s in 1..=STEPS {
        let t = s as f32 / STEPS as f32;
        let u = 1.0 - t;
        let px = u * u * ax + 2.0 * u * t * cx + t * t * bx;
        let py = u * u * ay + 2.0 * u * t * cy + t * t * by;
        out.push([px, py]);
    }
}

fn push_capsule(a: [f32; 2], b: [f32; 2], r: f32, col: [f32; 4], out: &mut Vec<ShapeInstance>) {
    let bx = a[0].min(b[0]) - r - 2.0;
    let by = a[1].min(b[1]) - r - 2.0;
    let bw = (a[0].max(b[0]) + r + 2.0) - bx;
    let bh = (a[1].max(b[1]) + r + 2.0) - by;
    out.push(ShapeInstance {
        bounds: [bx, by, bw, bh],
        p0: [a[0], a[1], b[0], b[1]],
        p1: [0.0; 4],
        fill: col,
        stroke: [0.0; 4],
        meta: [4.0, 0.0, r, 0.0],
    });
}

#[allow(clippy::too_many_arguments)]
pub fn bake_swf_text_gpu(
    gpu: &GpuCore,
    font: &SwfFont,
    size: f32,
    scale_x: f32,
    scale_y: f32,
    dpr: f32,
    color: (u8, u8, u8),
    stroke: Option<(u8, u8, u8)>,
    text: &str,
) -> Option<BakedGpuText> {
    let sx_log = SwfFont::scale_for_size(size) * scale_x;
    let sy_log = SwfFont::scale_for_size(size) * scale_y;
    let total_w: f32 = text
        .chars()
        .map(|c| {
            font.glyphs
                .get(&c)
                .map(|g| g.advance * sx_log)
                .unwrap_or(size * 0.45 * scale_x)
        })
        .sum();
    let stroke_w = (size * 0.14).clamp(2.0, 5.2);
    let pad = if stroke.is_some() {
        stroke_w.ceil() + 3.0
    } else {
        2.0
    };
    let sh_pad = if stroke.is_some() { 8.0 } else { 0.0 };
    let log_w = (total_w + pad * 2.0 + 4.0).ceil().max(1.0);
    let log_h = ((font.ascent + font.descent) * sy_log + pad * 2.0 + 6.0)
        .ceil()
        .max(1.0);

    let max_dim = gpu.max_tex_dim.min(4096) as f32;
    let max_scale = (max_dim / (log_w.max(log_h) + sh_pad * 2.0)).max(1.0);
    let eff_dpr = dpr.clamp(1.0, 4.0).min(max_scale);

    let pm_w = (log_w * eff_dpr).round().max(1.0) as u32;
    let pm_h = (log_h * eff_dpr).round().max(1.0) as u32;
    let sx = sx_log * eff_dpr;
    let sy = sy_log * eff_dpr;
    let pad_px = pad * eff_dpr;
    let base_y = font.ascent * sy + pad_px;
    let stroke_r = (stroke_w * eff_dpr) * 0.5;
    let sc_col = stroke.map(|s| {
        [
            s.0 as f32 / 255.0,
            s.1 as f32 / 255.0,
            s.2 as f32 / 255.0,
            1.0,
        ]
    });

    let mut fill_tris: Vec<GlyphVertex> = Vec::with_capacity(text.len() * 48);
    let mut stroke_caps: Vec<ShapeInstance> = Vec::new();
    let mut contour: Vec<[f32; 2]> = Vec::with_capacity(64);
    let mut pen_x = pad_px;

    let flush_contour =
        |pts: &mut Vec<[f32; 2]>, tris: &mut Vec<GlyphVertex>, caps: &mut Vec<ShapeInstance>| {
            if pts.len() >= 2 {
                if let Some(sc) = sc_col {
                    for w in pts.windows(2) {
                        push_capsule(w[0], w[1], stroke_r, sc, caps);
                    }
                    let last = pts[pts.len() - 1];
                    let first = pts[0];
                    if (last[0] - first[0]).abs() + (last[1] - first[1]).abs() > 1e-3 {
                        push_capsule(last, first, stroke_r, sc, caps);
                    }
                }
                if pts.len() >= 3 {
                    let anchor = pts[0];
                    for i in 1..(pts.len() - 1) {
                        tris.push(GlyphVertex { pos: anchor });
                        tris.push(GlyphVertex { pos: pts[i] });
                        tris.push(GlyphVertex { pos: pts[i + 1] });
                    }
                }
            }
            pts.clear();
        };

    for ch in text.chars() {
        let Some(glyph) = font.glyphs.get(&ch) else {
            pen_x += size * 0.45 * scale_x * eff_dpr;
            continue;
        };
        for &op in &glyph.ops {
            match op {
                GlyphOp::Move(gx, gy) => {
                    flush_contour(&mut contour, &mut fill_tris, &mut stroke_caps);
                    contour.push([pen_x + gx * sx, base_y + gy * sy]);
                }
                GlyphOp::Line(gx, gy) => {
                    contour.push([pen_x + gx * sx, base_y + gy * sy]);
                }
                GlyphOp::Quad(cx, cy, gx, gy) => {
                    if let Some(&prev) = contour.last() {
                        push_quad_points(
                            prev[0],
                            prev[1],
                            pen_x + cx * sx,
                            base_y + cy * sy,
                            pen_x + gx * sx,
                            base_y + gy * sy,
                            &mut contour,
                        );
                    }
                }
            }
        }
        flush_contour(&mut contour, &mut fill_tris, &mut stroke_caps);
        pen_x += glyph.advance * sx;
    }
    if fill_tris.is_empty() && stroke_caps.is_empty() {
        return None;
    }

    let msaa_tex = gpu.device.create_texture(&TextureDescriptor {
        label: Some("glyph_msaa"),
        size: Extent3d {
            width: pm_w,
            height: pm_h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 4,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let msaa_view = msaa_tex.create_view(&TextureViewDescriptor::default());
    let ds_tex = gpu.device.create_texture(&TextureDescriptor {
        label: Some("glyph_ds"),
        size: Extent3d {
            width: pm_w,
            height: pm_h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 4,
        dimension: TextureDimension::D2,
        format: TextureFormat::Depth24PlusStencil8,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let ds_view = ds_tex.create_view(&TextureViewDescriptor::default());
    let resolved = make_gpu_texture(
        gpu,
        pm_w,
        pm_h,
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
    );

    let g_uni = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some("glyph_u"),
        contents: bytemuck::bytes_of(&GlyphUniform {
            inv_half: [2.0 / pm_w as f32, 2.0 / pm_h as f32, 0.0, 0.0],
            color: [
                color.0 as f32 / 255.0,
                color.1 as f32 / 255.0,
                color.2 as f32 / 255.0,
                1.0,
            ],
        }),
        usage: BufferUsages::UNIFORM,
    });
    let g_bg = gpu.device.create_bind_group(&BindGroupDescriptor {
        label: Some("glyph_bg"),
        layout: &gpu.pipelines.glyph_bgl,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: g_uni.as_entire_binding(),
        }],
    });
    let (_sc_bg, so_bg) = make_stage_cmat_bg(gpu, pm_w as f32, pm_h as f32);

    let w = pm_w as f32;
    let h = pm_h as f32;
    let cover_verts = [
        GlyphVertex { pos: [0.0, 0.0] },
        GlyphVertex { pos: [w, 0.0] },
        GlyphVertex { pos: [w, h] },
        GlyphVertex { pos: [0.0, 0.0] },
        GlyphVertex { pos: [w, h] },
        GlyphVertex { pos: [0.0, h] },
    ];
    let fill_vb = (!fill_tris.is_empty()).then(|| {
        gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("glyph_fill_vb"),
            contents: bytemuck::cast_slice(&fill_tris),
            usage: BufferUsages::VERTEX,
        })
    });
    let cap_vb = (!stroke_caps.is_empty()).then(|| {
        gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("glyph_cap_vb"),
            contents: bytemuck::cast_slice(&stroke_caps),
            usage: BufferUsages::VERTEX,
        })
    });
    let cover_vb = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some("glyph_cover_vb"),
        contents: bytemuck::cast_slice(&cover_verts),
        usage: BufferUsages::VERTEX,
    });

    let mut enc = gpu.device.create_command_encoder(&Default::default());
    {
        let mut rp = enc.begin_render_pass(&RenderPassDescriptor {
            label: Some("glyph_msaa_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &msaa_view,
                resolve_target: Some(&resolved.view),
                ops: Operations {
                    load: LoadOp::Clear(Color::TRANSPARENT),
                    store: StoreOp::Discard,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &ds_view,
                depth_ops: None,
                stencil_ops: Some(Operations {
                    load: LoadOp::Clear(0),
                    store: StoreOp::Discard,
                }),
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if let Some(cvb) = &cap_vb {
            rp.set_pipeline(&gpu.pipelines.shape_msaa);
            rp.set_bind_group(0, &so_bg, &[]);
            rp.set_vertex_buffer(0, cvb.slice(..));
            rp.draw(0..6, 0..stroke_caps.len() as u32);
        }
        if let Some(fvb) = &fill_vb {
            rp.set_pipeline(&gpu.pipelines.glyph_stencil);
            rp.set_bind_group(0, &g_bg, &[]);
            rp.set_vertex_buffer(0, fvb.slice(..));
            rp.draw(0..fill_tris.len() as u32, 0..1);

            rp.set_pipeline(&gpu.pipelines.glyph_cover);
            rp.set_stencil_reference(0);
            rp.set_bind_group(0, &g_bg, &[]);
            rp.set_vertex_buffer(0, cover_vb.slice(..));
            rp.draw(0..6, 0..1);
        }
    }

    if sh_pad > 0.0 {
        let sh_pad_px = (sh_pad * eff_dpr).round().max(1.0) as u32;
        let blur_arg = (16.0 * eff_dpr).min(33.3);
        let sh = encode_blur_into(
            gpu,
            &mut enc,
            &resolved.view,
            pm_w,
            pm_h,
            4.0 * eff_dpr,
            0.0,
            (0, 0, 0),
            0.5,
            blur_arg,
            1.0,
            sh_pad_px,
        );
        let out = make_gpu_texture(
            gpu,
            sh.width,
            sh.height,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );
        let (csc_bg, _) = make_stage_cmat_bg(gpu, out.width as f32, out.height as f32);
        let insts = [
            SpriteInstance {
                row0: [1.0, 0.0, 0.0, sh.width as f32],
                row1: [0.0, 1.0, 0.0, sh.height as f32],
                uv_rect: [0.0, 0.0, 1.0, 1.0],
                params: [0.0, 1.0, 1.0, 0.0],
                tint: [1.0, 1.0, 1.0, 0.0],
            },
            SpriteInstance {
                row0: [1.0, 0.0, sh_pad_px as f32, pm_w as f32],
                row1: [0.0, 1.0, sh_pad_px as f32, pm_h as f32],
                uv_rect: [0.0, 0.0, 1.0, 1.0],
                params: [0.0, 1.0, 1.0, 0.0],
                tint: [1.0, 1.0, 1.0, 0.0],
            },
        ];
        let comp_vb = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("glyph_comp_vb"),
            contents: bytemuck::cast_slice(&insts),
            usage: BufferUsages::VERTEX,
        });
        {
            let mut rp = enc.begin_render_pass(&RenderPassDescriptor {
                label: Some("glyph_comp_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &out.view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::TRANSPARENT),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&gpu.pipelines.sprite_unorm);
            rp.set_bind_group(0, &csc_bg, &[]);
            rp.set_vertex_buffer(0, comp_vb.slice(..));
            rp.set_bind_group(1, &sh.bind_group, &[]);
            rp.draw(0..6, 0..1);
            rp.set_bind_group(1, &resolved.bind_group, &[]);
            rp.draw(0..6, 1..2);
        }
        gpu.queue.submit(Some(enc.finish()));
        return Some(BakedGpuText {
            logical_w: out.width as f32 / eff_dpr,
            logical_h: out.height as f32 / eff_dpr,
            origin_x: pad + sh_pad_px as f32 / eff_dpr,
            origin_y: pad + sh_pad_px as f32 / eff_dpr,
            texture: out,
        });
    }

    gpu.queue.submit(Some(enc.finish()));
    Some(BakedGpuText {
        logical_w: pm_w as f32 / eff_dpr,
        logical_h: pm_h as f32 / eff_dpr,
        origin_x: pad,
        origin_y: pad,
        texture: resolved,
    })
}
