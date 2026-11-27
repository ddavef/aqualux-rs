// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::{GameColor, PieceShape, PieceType, Tile};
use crate::render::board::BoardAnim;
use crate::render::canvas::{BoardLayout, Canvas};
use crate::render::easing::{bounce_out, quad_out, strong_in};
use crate::render::pipe_mask::draw_flow_timer;
use crate::render::tex_bank::TexBank;
use crate::render::text::FontSet;

pub(crate) fn tile_bitmap_keys(tile: &Tile) -> (&'static str, &'static str) {
    match (tile.piece_type, tile.shape) {
        (PieceType::Blocker, _) => ("CementBlockerBitmap", "CementBlockerBitmap"),
        (PieceType::Drain, _) | (PieceType::Portal, _) => {
            ("DrainPipeClearBitmap", "DrainPipeFullBitmap")
        }
        (PieceType::Source, _) | (_, PieceShape::Spigot) => {
            ("SpigotClearBitmap", "SpigotFullBitmap")
        }
        (_, PieceShape::Elbow) => ("ElbowClearBitmap", "ElbowFullBitmap"),
        (_, PieceShape::Straight) => ("StraightClearBitmap", "StraightFullBitmap"),
        (_, PieceShape::Cross) => ("CrossClearBitmap", "CrossFullBitmap"),
        _ => ("ElbowClearBitmap", "ElbowFullBitmap"),
    }
}

pub(crate) fn frost_bitmap_key(shape: PieceShape) -> Option<&'static str> {
    match shape {
        PieceShape::Straight => Some("StraightFrostBitmap"),
        PieceShape::Cross => Some("CrossFrostBitmap"),
        PieceShape::Elbow => Some("ElbowFrostBitmap"),
        _ => None,
    }
}

fn slime_uv(frame: u32) -> [f32; 4] {
    let f = frame.min(25);
    let col = (f % 20) as f32;
    let row = (f / 20) as f32;
    [
        (col * 128.0 + 0.5) / 2560.0,
        (row * 128.0 + 0.5) / 256.0,
        127.0 / 2560.0,
        127.0 / 256.0,
    ]
}

#[allow(clippy::too_many_arguments)]
/// Start sign `(59, -19)` on Source and Goal sign `(-36, -33)` on Drain behind pipe sprites.
pub(crate) fn draw_attachment_underlay(
    canvas: &mut Canvas,
    bank: &TexBank,
    tile: &Tile,
    cx: f32,
    cy: f32,
    tpx: f32,
    alpha: f32,
    mirror_x: bool,
    blur: (f32, f32),
) {
    let scale = tpx / 128.0;
    let (key, ox, oy) = match tile.piece_type {
        PieceType::Source => ("StartSignBitmap", 59.0, -19.0),
        PieceType::Drain => ("GoalSignBitmap", -36.0, -33.0),
        _ => return,
    };
    if let Some((tid, sw, sh)) = bank.named(key) {
        let w = sw as f32 * scale;
        let h = sh as f32 * scale;
        let rel_cx = (ox + sw as f32 * 0.5 - 64.0) * scale;
        let rel_cy = (oy + sh as f32 * 0.5 - 64.0) * scale;
        let sx = if mirror_x { cx - rel_cx } else { cx + rel_cx };
        let sy = cy + rel_cy;
        canvas.draw_tex_uv_rotated_blur(
            tid,
            [0.0, 0.0, 1.0, 1.0],
            sx,
            sy,
            w,
            h,
            0.0,
            None,
            1.0,
            alpha,
            mirror_x,
            blur,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_drain_underlay(
    canvas: &mut Canvas,
    bank: &TexBank,
    tile: &Tile,
    cx: f32,
    cy: f32,
    tile_px: f32,
    alpha: f32,
    mirror_x: bool,
    blur: (f32, f32),
) {
    let uv_full = [0.0, 0.0, 1.0, 1.0];
    if let Some((tid, _, _)) = bank.named("DrainBaseBitmap") {
        canvas.draw_tex_uv_rotated_blur(
            tid,
            uv_full,
            cx,
            cy,
            tile_px,
            tile_px,
            tile.visual_rot_deg,
            None,
            1.0,
            alpha,
            mirror_x,
            blur,
        );
    }
    if let Some(anim) = tile.slime_anim
        && let Some((tid, _, _)) = bank.named("DrainingSlimeSheet")
    {
        let frame = ((anim * 15.0).floor() as u32).min(25);
        canvas.draw_tex_uv_rotated_blur(
            tid,
            slime_uv(frame),
            cx,
            cy,
            tile_px,
            tile_px,
            tile.visual_rot_deg,
            Some((tile.liquid_color, false)),
            1.0,
            alpha,
            mirror_x,
            blur,
        );
        canvas.boost_last_sprite(1.28);
    }
    if let Some((tid, _, _)) = bank.named("DrainRingBitmap") {
        canvas.draw_tex_uv_rotated_blur(
            tid,
            uv_full,
            cx,
            cy,
            tile_px,
            tile_px,
            tile.visual_rot_deg,
            Some((tile.pipe_color, false)),
            1.0,
            alpha,
            mirror_x,
            blur,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_portal_underlay(
    canvas: &mut Canvas,
    bank: &TexBank,
    tile: &Tile,
    cx: f32,
    cy: f32,
    tile_px: f32,
    alpha: f32,
    mirror_x: bool,
    time: f32,
    blur: (f32, f32),
) {
    let portal_col = tile.secondary_color.unwrap_or(GameColor::Violet);
    let p_scale = 1.0 - 0.1 * quad_out((time / 2.0).clamp(0.0, 1.0));
    let psz = tile_px * p_scale;
    if let Some((tid, _, _)) = bank.named("YellowPortalBitmap") {
        canvas.draw_tex_uv_rotated_blur(
            tid,
            [0.0, 0.0, 1.0, 1.0],
            cx,
            cy,
            psz,
            psz,
            tile.visual_rot_deg + time * 60.0,
            Some((portal_col, false)),
            1.0,
            alpha,
            mirror_x,
            blur,
        );
        canvas.boost_last_sprite(1.28);
    }
    if let Some(anim) = tile.slime_anim
        && let Some((tid, _, _)) = bank.named("DrainingPortalSlimeSheet")
    {
        let frame = ((anim * 15.0).floor() as u32).min(25);
        canvas.draw_tex_uv_rotated_blur(
            tid,
            slime_uv(frame),
            cx,
            cy,
            tile_px,
            tile_px,
            tile.visual_rot_deg,
            Some((tile.liquid_color, false)),
            1.0,
            alpha,
            mirror_x,
            blur,
        );
        canvas.boost_last_sprite(1.28);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_tile_timer_overlay(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    tile: &Tile,
    cx: f32,
    cy: f32,
    tpx: f32,
    alpha: f32,
) {
    if let Some(ref timer) = tile.timer {
        let tl_x = cx - tpx * 0.5;
        let tl_y = cy - tpx * 0.5;
        draw_flow_timer(canvas, bank, fonts, tl_x + 20.0, tl_y + 20.0, timer, alpha);
    }
}

pub(crate) fn tile_xform(
    tile: &Tile,
    hide_seq: usize,
    cols: usize,
    max_filled_delay: f32,
    layout: BoardLayout,
    mirror_x: bool,
    anim: BoardAnim,
) -> (f32, f32, f32, f32, f32) {
    let draw_x = if mirror_x {
        cols.saturating_sub(1 + tile.x)
    } else {
        tile.x
    };
    let (base_cx, base_cy) = layout.tile_center(draw_x, tile.y);
    let mx = if mirror_x {
        -tile.swap_offset.0
    } else {
        tile.swap_offset.0
    };
    let cx = base_cx + mx * layout.tile_px;
    let mut cy = base_cy + tile.swap_offset.1 * layout.tile_px;
    match anim {
        BoardAnim::PopIn {
            elapsed,
            start_delay,
        } => {
            let col_ord = if tile.y.is_multiple_of(2) {
                tile.x
            } else {
                cols.saturating_sub(1 + tile.x)
            };
            let delay = start_delay + (tile.y * cols + col_ord) as f32 * 0.1;
            if elapsed < delay {
                return (cx, cy, 0.0, 0.0, 0.0);
            }
            let u = ((elapsed - delay) / 0.6).clamp(0.0, 1.0);
            let b = bounce_out(u);
            (
                cx,
                cy,
                layout.tile_px * (0.1 + 0.9 * b),
                b.clamp(0.0, 1.0),
                ((1.0 - b) * 8.0).max(0.0),
            )
        }
        BoardAnim::Obscured { alpha } => (cx, cy, layout.tile_px, alpha, (1.0 - alpha) * 32.0),
        BoardAnim::Preview { alpha } => (cx, cy, layout.tile_px, alpha, 0.0),
        BoardAnim::HideDrop { elapsed, delay } => {
            let tdelay = if tile.has_filled {
                max_filled_delay
            } else {
                delay + hide_seq.min(11) as f32 * 0.15
            };
            let mut blur_px = 0.0;
            if elapsed > tdelay {
                let u = ((elapsed - tdelay) / 0.8).clamp(0.0, 1.0);
                let e = strong_in(u);
                cy += (800.0 - base_cy) * e;
                blur_px = e * 16.0;
            }
            (cx, cy, layout.tile_px, 1.0, blur_px)
        }
        BoardAnim::Active => (cx, cy, layout.tile_px, 1.0, 0.0),
    }
}
