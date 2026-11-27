// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::Matrix2D;
use crate::game::GameColor;
use crate::render::gpu_types::{
    LeakUniform, PipeMaskInstance, ShapeInstance, SpriteInstance, TexId, color_mat_id,
};
pub use crate::render::layout::{BoardLayout, Rect};
use std::sync::Arc;
use wgpu::BindGroup;

pub const LOGICAL_W: usize = 800;
pub const LOGICAL_H: usize = 600;

pub enum DrawCmd {
    Clear(u8, u8, u8),
    SetClip(Option<Rect>),
    SpriteTex(TexId, SpriteInstance),
    SpriteDirect(Arc<BindGroup>, SpriteInstance),
    Shape(ShapeInstance),
    PipeMask(TexId, PipeMaskInstance),
    Leak(Box<LeakUniform>),
}

pub struct Canvas {
    pub cmds: Vec<DrawCmd>,
    pub dpr: f32,
    last_sprite_pushed: bool,
}

fn rgba_f(c: (u8, u8, u8, u8)) -> [f32; 4] {
    [
        c.0 as f32 / 255.0,
        c.1 as f32 / 255.0,
        c.2 as f32 / 255.0,
        c.3 as f32 / 255.0,
    ]
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            cmds: Vec::with_capacity(512),
            dpr: 1.0,
            last_sprite_pushed: false,
        }
    }

    pub fn begin_frame(&mut self, dpr: f32) {
        self.cmds.clear();
        self.dpr = dpr.max(1.0);
        self.last_sprite_pushed = false;
    }

    pub fn clear(&mut self, rgb: (u8, u8, u8)) {
        self.last_sprite_pushed = false;
        self.cmds.push(DrawCmd::Clear(rgb.0, rgb.1, rgb.2));
    }

    pub fn set_clip(&mut self, rect: Option<Rect>) {
        self.last_sprite_pushed = false;
        self.cmds.push(DrawCmd::SetClip(rect));
    }

    pub fn fill_rect_alpha(&mut self, rect: Rect, r: u8, g: u8, b: u8, a: u8) {
        self.last_sprite_pushed = false;
        if a == 0 || rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        self.cmds.push(DrawCmd::Shape(ShapeInstance {
            bounds: [rect.x, rect.y, rect.w, rect.h],
            p0: [0.0; 4],
            p1: [0.0; 4],
            fill: rgba_f((r, g, b, a)),
            stroke: [0.0; 4],
            meta: [0.0; 4],
        }));
    }

    pub fn draw_rounded_rect(
        &mut self,
        rect: Rect,
        radius: f32,
        fill: (u8, u8, u8, u8),
        border: Option<(u8, u8, u8, u8, f32)>,
    ) {
        self.last_sprite_pushed = false;
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let (sr, sg, sb, sa, st_w) = border.unwrap_or((0, 0, 0, 0, 0.0));
        let pad = (st_w * 0.5 + 2.0).max(2.0);
        self.cmds.push(DrawCmd::Shape(ShapeInstance {
            bounds: [
                rect.x - pad,
                rect.y - pad,
                rect.w + pad * 2.0,
                rect.h + pad * 2.0,
            ],
            p0: [rect.x, rect.y, rect.w, rect.h],
            p1: [0.0; 4],
            fill: rgba_f(fill),
            stroke: rgba_f((sr, sg, sb, sa)),
            meta: [1.0, st_w, radius.max(0.0), 0.0],
        }));
    }

    pub fn draw_modal_bubble(&mut self, rect: Rect) {
        self.last_sprite_pushed = false;
        let pad = 6.0;
        self.cmds.push(DrawCmd::Shape(ShapeInstance {
            bounds: [
                rect.x - pad,
                rect.y - pad,
                rect.w + pad * 2.0,
                rect.h + pad * 2.0,
            ],
            p0: [rect.x, rect.y, rect.w, rect.h],
            p1: [0.0; 4],
            fill: rgba_f((0x1C, 0x4A, 0x84, 247)),
            stroke: rgba_f((0, 0, 0, 247)),
            meta: [2.0, 4.0, 17.5, 0.0],
        }));
    }

    pub fn fill_circle_alpha(&mut self, cx: f32, cy: f32, radius: f32, rgba: (u8, u8, u8, u8)) {
        self.last_sprite_pushed = false;
        if radius <= 0.0 || rgba.3 == 0 {
            return;
        }
        let p = radius + 2.0;
        self.cmds.push(DrawCmd::Shape(ShapeInstance {
            bounds: [cx - p, cy - p, p * 2.0, p * 2.0],
            p0: [cx, cy, 0.0, 0.0],
            p1: [0.0; 4],
            fill: rgba_f(rgba),
            stroke: [0.0; 4],
            meta: [3.0, 0.0, radius, 0.0],
        }));
    }

    pub fn stroke_circle_alpha(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        thickness: f32,
        rgba: (u8, u8, u8, u8),
    ) {
        self.last_sprite_pushed = false;
        if radius <= 0.0 || thickness <= 0.0 || rgba.3 == 0 {
            return;
        }
        let p = radius + thickness * 0.5 + 2.0;
        self.cmds.push(DrawCmd::Shape(ShapeInstance {
            bounds: [cx - p, cy - p, p * 2.0, p * 2.0],
            p0: [cx, cy, 0.0, 0.0],
            p1: [0.0; 4],
            fill: [0.0; 4],
            stroke: rgba_f(rgba),
            meta: [3.0, thickness, radius, 0.0],
        }));
    }

    pub fn draw_star_glow(&mut self, cx: f32, cy: f32, radius: f32, opacity: f32) {
        self.last_sprite_pushed = false;
        let op = opacity.clamp(0.0, 1.0);
        if radius <= 0.0 || op <= 0.004 {
            return;
        }
        let p = radius + 2.0;
        self.cmds.push(DrawCmd::Shape(ShapeInstance {
            bounds: [cx - p, cy - p, p * 2.0, p * 2.0],
            p0: [cx, cy, 0.0, 0.0],
            p1: [0.0; 4],
            fill: [1.0, 1.0, 1.0, op],
            stroke: [0.0; 4],
            meta: [3.0, 0.0, radius, 1.0],
        }));
    }

    pub fn draw_star_orb(&mut self, cx: f32, cy: f32, radius: f32, opacity: f32) {
        self.last_sprite_pushed = false;
        let op = opacity.clamp(0.0, 1.0);
        if radius <= 0.0 || op <= 0.004 {
            return;
        }
        let p = radius + 2.0;
        self.cmds.push(DrawCmd::Shape(ShapeInstance {
            bounds: [cx - p, cy - p, p * 2.0, p * 2.0],
            p0: [cx, cy, 0.0, 0.0],
            p1: [0.0; 4],
            fill: [1.0, 1.0, 1.0, op],
            stroke: [0.0; 4],
            meta: [3.0, 0.0, radius, 2.0],
        }));
    }

    pub fn draw_line(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        thickness: f32,
        rgba: (u8, u8, u8, u8),
    ) {
        self.last_sprite_pushed = false;
        if rgba.3 == 0 || thickness <= 0.0 {
            return;
        }
        let r = thickness * 0.5;
        let pad = r + 2.0;
        let bx = x0.min(x1) - pad;
        let by = y0.min(y1) - pad;
        self.cmds.push(DrawCmd::Shape(ShapeInstance {
            bounds: [bx, by, (x0.max(x1) + pad) - bx, (y0.max(y1) + pad) - by],
            p0: [x0, y0, x1, y1],
            p1: [0.0; 4],
            fill: rgba_f(rgba),
            stroke: [0.0; 4],
            meta: [4.0, 0.0, r, 0.0],
        }));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_tutorial_bubble(
        &mut self,
        bx: f32,
        by: f32,
        bw: f32,
        bh: f32,
        radius: f32,
        stroke_w: f32,
        tail_cx: f32,
        tail_hw: f32,
        tri_h: f32,
        bevel_scale: f32,
    ) {
        self.last_sprite_pushed = false;
        let pad = stroke_w * 0.5 + 3.0;
        self.cmds.push(DrawCmd::Shape(ShapeInstance {
            bounds: [bx - pad, by - pad, bw + pad * 2.0, bh + tri_h + pad * 2.0],
            p0: [bx, by, bw, bh],
            p1: [tail_cx, tail_hw, tri_h, 0.0],
            fill: rgba_f((0xEE, 0xEE, 0xEE, 255)),
            stroke: rgba_f((0, 0, 0, 255)),
            meta: [5.0, stroke_w, radius, bevel_scale],
        }));
    }

    pub fn draw_rainbow_tex(&mut self, tid: TexId, t_end: f32, alpha: f32, blur: (f32, f32)) {
        self.last_sprite_pushed = false;
        let a = alpha.clamp(0.0, 1.0);
        if a <= 0.01 || t_end <= 0.005 {
            return;
        }
        let u_end = (t_end * t_end).clamp(0.005, 1.0);
        let w = u_end * LOGICAL_W as f32;
        self.cmds.push(DrawCmd::SpriteTex(
            tid,
            SpriteInstance {
                row0: [1.0, 0.0, 0.0, w],
                row1: [0.0, 1.0, 120.0, 480.0],
                uv_rect: [0.0, 0.2, u_end, 0.8],
                params: [0.0, 1.0, a, blur.0],
                tint: [1.0, 1.0, 1.0, blur.1],
            },
        ));
        self.last_sprite_pushed = true;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_tex_rect_blur(
        &mut self,
        tid: TexId,
        rect: Rect,
        color: Option<(GameColor, bool)>,
        shade: f32,
        alpha: f32,
        blur: (f32, f32),
    ) {
        self.last_sprite_pushed = false;
        let a = alpha.clamp(0.0, 1.0);
        if a <= 0.004 || rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        self.cmds.push(DrawCmd::SpriteTex(
            tid,
            SpriteInstance {
                row0: [1.0, 0.0, rect.x, rect.w],
                row1: [0.0, 1.0, rect.y, rect.h],
                uv_rect: [0.0, 0.0, 1.0, 1.0],
                params: [color_mat_id(color) as f32, shade, a, blur.0],
                tint: [1.0, 1.0, 1.0, blur.1],
            },
        ));
        self.last_sprite_pushed = true;
    }

    pub fn draw_tex_rect(
        &mut self,
        tid: TexId,
        rect: Rect,
        color: Option<(GameColor, bool)>,
        shade: f32,
        alpha: f32,
    ) {
        self.draw_tex_rect_blur(tid, rect, color, shade, alpha, (0.0, 0.0));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_tex_uv_rotated_blur(
        &mut self,
        tid: TexId,
        uv_rect: [f32; 4],
        cx: f32,
        cy: f32,
        draw_w: f32,
        draw_h: f32,
        rot_deg_cw: f32,
        color: Option<(GameColor, bool)>,
        shade: f32,
        alpha: f32,
        mirror_x: bool,
        blur: (f32, f32),
    ) {
        self.last_sprite_pushed = false;
        let a = alpha.clamp(0.0, 1.0);
        if a <= 0.004 || draw_w <= 0.0 || draw_h <= 0.0 {
            return;
        }
        let (r0, r1) = rotated_rows(cx, cy, draw_w, draw_h, rot_deg_cw, mirror_x);
        self.cmds.push(DrawCmd::SpriteTex(
            tid,
            SpriteInstance {
                row0: r0,
                row1: r1,
                uv_rect,
                params: [color_mat_id(color) as f32, shade, a, blur.0],
                tint: [1.0, 1.0, 1.0, blur.1],
            },
        ));
        self.last_sprite_pushed = true;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_tex_matrix(
        &mut self,
        tid: TexId,
        m: Matrix2D,
        w: f32,
        h: f32,
        color: Option<(GameColor, bool)>,
        shade: f32,
        alpha: f32,
    ) {
        self.last_sprite_pushed = false;
        let a = alpha.clamp(0.0, 1.0);
        if a <= 0.004 || w <= 0.0 || h <= 0.0 {
            return;
        }
        self.cmds.push(DrawCmd::SpriteTex(
            tid,
            SpriteInstance {
                row0: [m.a, m.c, m.tx, w],
                row1: [m.b, m.d, m.ty, h],
                uv_rect: [0.0, 0.0, 1.0, 1.0],
                params: [color_mat_id(color) as f32, shade, a, 0.0],
                tint: [1.0, 1.0, 1.0, 0.0],
            },
        ));
        self.last_sprite_pushed = true;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_direct_sprite_fx(
        &mut self,
        bg: Arc<BindGroup>,
        row0: [f32; 4],
        row1: [f32; 4],
        brightness: f32,
        alpha: f32,
        tint_rgb: [f32; 3],
        blur: (f32, f32),
    ) {
        self.last_sprite_pushed = false;
        let a = alpha.clamp(0.0, 1.0);
        if a <= 0.004 {
            return;
        }
        self.cmds.push(DrawCmd::SpriteDirect(
            bg,
            SpriteInstance {
                row0,
                row1,
                uv_rect: [0.0, 0.0, 1.0, 1.0],
                params: [0.0, brightness, a, blur.0],
                tint: [tint_rgb[0], tint_rgb[1], tint_rgb[2], blur.1],
            },
        ));
        self.last_sprite_pushed = true;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_pipe_masked(
        &mut self,
        tid: TexId,
        cx: f32,
        cy: f32,
        draw_w: f32,
        draw_h: f32,
        rot_deg_cw: f32,
        mirror_x: bool,
        caps: [[f32; 4]; 4],
        radii: [f32; 4],
        count: u32,
        color: GameColor,
        alpha: f32,
        blur: (f32, f32),
    ) {
        self.last_sprite_pushed = false;
        let a = alpha.clamp(0.0, 1.0);
        if a <= 0.004 || count == 0 || draw_w <= 0.0 || draw_h <= 0.0 {
            return;
        }
        let (row0, row1) = rotated_rows(cx, cy, draw_w, draw_h, rot_deg_cw, mirror_x);
        self.cmds.push(DrawCmd::PipeMask(
            tid,
            PipeMaskInstance {
                row0,
                row1,
                cap0: caps[0],
                cap1: caps[1],
                cap2: caps[2],
                cap3: caps[3],
                radii,
                params: [
                    color_mat_id(Some((color, false))) as f32,
                    a,
                    count as f32,
                    blur.0.max(blur.1).max(0.0),
                ],
            },
        ));
    }

    pub fn draw_leak(&mut self, uni: LeakUniform) {
        self.last_sprite_pushed = false;
        self.cmds.push(DrawCmd::Leak(Box::new(uni)));
    }

    pub fn boost_last_sprite(&mut self, boost: f32) {
        if !std::mem::replace(&mut self.last_sprite_pushed, false) {
            return;
        }
        let mult = boost.max(1.0);
        if let Some(DrawCmd::SpriteTex(_, inst) | DrawCmd::SpriteDirect(_, inst)) =
            self.cmds.last_mut()
        {
            inst.tint[0] *= mult;
            inst.tint[1] *= mult;
            inst.tint[2] *= mult;
        }
    }

    pub fn sheen_last_sprite(&mut self, boost: f32) {
        if !std::mem::replace(&mut self.last_sprite_pushed, false) {
            return;
        }
        let neg = -boost.max(1.0);
        if let Some(DrawCmd::SpriteTex(_, inst) | DrawCmd::SpriteDirect(_, inst)) =
            self.cmds.last_mut()
        {
            inst.tint[0] = neg;
            inst.tint[1] = neg;
            inst.tint[2] = neg;
        }
    }

    pub fn transform_cmds_since(
        &mut self,
        start_idx: usize,
        cx: f32,
        cy: f32,
        scale: f32,
        alpha: f32,
    ) {
        self.last_sprite_pushed = false;
        let a = alpha.clamp(0.0, 1.0);
        let s = scale.max(0.0);
        if a <= 0.004 || s <= 0.001 {
            self.cmds.truncate(start_idx);
            return;
        }
        if (s - 1.0).abs() < 1e-5 && (a - 1.0).abs() < 1e-5 {
            return;
        }
        let tx = |v: f32| cx + (v - cx) * s;
        let ty = |v: f32| cy + (v - cy) * s;
        for cmd in &mut self.cmds[start_idx..] {
            match cmd {
                DrawCmd::SetClip(Some(r)) => {
                    r.x = tx(r.x);
                    r.y = ty(r.y);
                    r.w *= s;
                    r.h *= s;
                }
                DrawCmd::SpriteTex(_, sp) | DrawCmd::SpriteDirect(_, sp) => {
                    sp.row0[0] *= s;
                    sp.row0[1] *= s;
                    sp.row0[2] = tx(sp.row0[2]);
                    sp.row1[0] *= s;
                    sp.row1[1] *= s;
                    sp.row1[2] = ty(sp.row1[2]);
                    sp.params[2] = (sp.params[2] * a).clamp(0.0, 1.0);
                }
                DrawCmd::Shape(sh) => {
                    sh.bounds[0] = tx(sh.bounds[0]);
                    sh.bounds[1] = ty(sh.bounds[1]);
                    sh.bounds[2] *= s;
                    sh.bounds[3] *= s;
                    sh.fill[3] *= a;
                    sh.stroke[3] *= a;
                    match sh.meta[0].round() as u32 {
                        1 | 2 => {
                            sh.p0[0] = tx(sh.p0[0]);
                            sh.p0[1] = ty(sh.p0[1]);
                            sh.p0[2] *= s;
                            sh.p0[3] *= s;
                            sh.meta[1] *= s;
                            sh.meta[2] *= s;
                        }
                        3 => {
                            sh.p0[0] = tx(sh.p0[0]);
                            sh.p0[1] = ty(sh.p0[1]);
                            sh.meta[1] *= s;
                            sh.meta[2] *= s;
                        }
                        4 => {
                            sh.p0[0] = tx(sh.p0[0]);
                            sh.p0[1] = ty(sh.p0[1]);
                            sh.p0[2] = tx(sh.p0[2]);
                            sh.p0[3] = ty(sh.p0[3]);
                            sh.meta[2] *= s;
                        }
                        5 => {
                            sh.p0[0] = tx(sh.p0[0]);
                            sh.p0[1] = ty(sh.p0[1]);
                            sh.p0[2] *= s;
                            sh.p0[3] *= s;
                            sh.p1[0] = tx(sh.p1[0]);
                            sh.p1[1] *= s;
                            sh.p1[2] *= s;
                            sh.meta[1] *= s;
                            sh.meta[2] *= s;
                            sh.meta[3] *= s;
                        }
                        _ => {}
                    }
                }
                DrawCmd::PipeMask(_, pm) => {
                    pm.row0[0] *= s;
                    pm.row0[1] *= s;
                    pm.row0[2] = tx(pm.row0[2]);
                    pm.row1[0] *= s;
                    pm.row1[1] *= s;
                    pm.row1[2] = ty(pm.row1[2]);
                    pm.params[1] = (pm.params[1] * a).clamp(0.0, 1.0);
                }
                DrawCmd::Clear(..) | DrawCmd::SetClip(None) | DrawCmd::Leak(_) => {}
            }
        }
    }
}

fn rotated_rows(
    cx: f32,
    cy: f32,
    dw: f32,
    dh: f32,
    rot_deg_cw: f32,
    mirror_x: bool,
) -> ([f32; 4], [f32; 4]) {
    let sx = if mirror_x { -1.0 } else { 1.0 };
    let (si, co) = rot_deg_cw.to_radians().sin_cos();
    let (a, c) = (co * sx, -si * sx);
    let (b, d) = (si, co);
    let tx = cx - a * (dw * 0.5) - c * (dh * 0.5);
    let ty = cy - b * (dw * 0.5) - d * (dh * 0.5);
    ([a, c, tx, dw], [b, d, ty, dh])
}
