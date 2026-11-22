// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::{GameColor, PieceType, Tile};
use crate::render::board_underlay::{
    draw_attachment_underlay, draw_drain_underlay, draw_portal_underlay, draw_tile_timer_overlay,
    frost_bitmap_key, tile_bitmap_keys, tile_xform,
};
use crate::render::canvas::{BoardLayout, Canvas};
use crate::render::emitter::LeakEmitter;
use crate::render::pipe_mask::build_pipe_capsules;
use crate::render::tex_bank::{GlowPreset, PATH_HL_PAD, TexBank};
use crate::render::text::{FontFamily, FontSet, TextAlign};
use rustc_hash::FxHashSet;

#[derive(Debug, Clone, Copy)]
pub enum BoardAnim {
    PopIn { elapsed: f32, start_delay: f32 },
    Active,
    Obscured { alpha: f32 },
    HideDrop { elapsed: f32, delay: f32 },
}

pub struct BoardRenderer;

impl BoardRenderer {
    pub fn new() -> Self {
        Self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_board(
        &mut self,
        canvas: &mut Canvas,
        bank: &TexBank,
        fonts: &FontSet,
        tiles: &[Vec<Tile>],
        layout: BoardLayout,
        selected: Option<(usize, usize)>,
        swap_eligible: &[(usize, usize)],
        hover: Option<(usize, usize)>,
        tutorial_tiles: &[(usize, usize)],
        leaks: &[LeakEmitter],
        color_blind: bool,
        mirror_x: bool,
        anim: BoardAnim,
        time: f32,
    ) {
        let eligible_set: FxHashSet<(usize, usize)> = swap_eligible.iter().copied().collect();
        let tut_set: FxHashSet<(usize, usize)> = tutorial_tiles.iter().copied().collect();

        let rows = tiles.len();
        let cols = tiles.first().map(|r| r.len()).unwrap_or(0);
        let max_filled_delay = (0.15 * (rows * cols) as f32).min(0.15 * 12.0);
        let mut deferred_leakers = Vec::new();
        let mut hide_seq = 0usize;

        for row in tiles {
            for tile in row {
                let (cx, cy, tpx, alpha, blur_px) = tile_xform(
                    tile,
                    hide_seq,
                    cols,
                    max_filled_delay,
                    layout,
                    mirror_x,
                    anim,
                );
                let seq_here = hide_seq;
                if matches!(anim, BoardAnim::HideDrop { .. }) {
                    hide_seq += 1;
                }
                if alpha <= 0.01 || tpx <= 1.0 {
                    continue;
                }
                let pos = (tile.x, tile.y);
                if leaks.iter().any(|l| l.tile_pos == pos) {
                    deferred_leakers.push((tile, seq_here));
                    continue;
                }
                self.draw_tile_layer(
                    canvas,
                    bank,
                    fonts,
                    tile,
                    cx,
                    cy,
                    tpx,
                    alpha,
                    (blur_px, blur_px),
                    selected == Some(pos),
                    hover == Some(pos) && selected != Some(pos),
                    eligible_set.contains(&pos),
                    tut_set.contains(&pos),
                    &[],
                    color_blind,
                    mirror_x,
                    time,
                );
            }
        }

        for (tile, seq_here) in deferred_leakers {
            let (cx, cy, tpx, alpha, blur_px) = tile_xform(
                tile,
                seq_here,
                cols,
                max_filled_delay,
                layout,
                mirror_x,
                anim,
            );
            if alpha <= 0.01 || tpx <= 1.0 {
                continue;
            }
            let pos = (tile.x, tile.y);
            self.draw_tile_layer(
                canvas,
                bank,
                fonts,
                tile,
                cx,
                cy,
                tpx,
                alpha,
                (blur_px, blur_px),
                selected == Some(pos),
                hover == Some(pos) && selected != Some(pos),
                eligible_set.contains(&pos),
                tut_set.contains(&pos),
                leaks,
                color_blind,
                mirror_x,
                time,
            );
        }

        hide_seq = 0;
        for row in tiles {
            for tile in row {
                let (cx, cy, tpx, alpha, _) = tile_xform(
                    tile,
                    hide_seq,
                    cols,
                    max_filled_delay,
                    layout,
                    mirror_x,
                    anim,
                );
                if matches!(anim, BoardAnim::HideDrop { .. }) {
                    hide_seq += 1;
                }
                if alpha > 0.01 && tpx > 1.0 {
                    draw_tile_timer_overlay(canvas, bank, fonts, tile, cx, cy, tpx, alpha, time);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_tile_layer(
        &mut self,
        canvas: &mut Canvas,
        bank: &TexBank,
        fonts: &FontSet,
        tile: &Tile,
        cx: f32,
        cy: f32,
        tile_px: f32,
        alpha: f32,
        blur: (f32, f32),
        is_sel: bool,
        is_hov: bool,
        is_elig: bool,
        is_tut: bool,
        leaks: &[LeakEmitter],
        color_blind: bool,
        mirror_x: bool,
        time: f32,
    ) {
        let uv_full = [0.0, 0.0, 1.0, 1.0];
        let (clear_key, full_key) = tile_bitmap_keys(tile);
        draw_tile_filter_halo(
            canvas,
            bank,
            clear_key,
            cx,
            cy,
            tile_px,
            alpha,
            blur,
            tile.visual_rot_deg,
            mirror_x,
            is_sel,
            is_tut,
            is_hov && (tile.is_moveable || tile.can_rotate),
            is_elig,
        );

        for l in leaks {
            if l.tile_pos == (tile.x, tile.y) {
                let (dx, dy) = l.exit_side.delta();
                l.draw_at(
                    canvas,
                    cx + dx as f32 * tile_px * 0.5,
                    cy + dy as f32 * tile_px * 0.5,
                );
            }
        }

        if tile.piece_type == PieceType::Blocker {
            if let Some((tid, _, _)) = bank.named("CementBlockerBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid, uv_full, cx, cy, tile_px, tile_px, 0.0, None, 1.0, alpha, false, blur,
                );
            }
            return;
        }

        draw_attachment_underlay(canvas, bank, tile, cx, cy, tile_px, alpha, blur);
        if tile.piece_type == PieceType::Drain {
            draw_drain_underlay(canvas, bank, tile, cx, cy, tile_px, alpha, mirror_x, blur);
        } else if tile.piece_type == PieceType::Portal {
            draw_portal_underlay(
                canvas, bank, tile, cx, cy, tile_px, alpha, mirror_x, time, blur,
            );
        }

        if !tile.fills.is_empty()
            && let Some((tid, _, _)) = bank.named(full_key)
        {
            let (caps, radii, count) = build_pipe_capsules(&tile.fills, tile.direction);
            canvas.draw_pipe_masked(
                tid,
                cx,
                cy,
                tile_px,
                tile_px,
                tile.visual_rot_deg,
                mirror_x,
                caps,
                radii,
                count,
                tile.liquid_color,
                alpha,
            );
        }

        if let Some(ref hl) = tile.path_hl {
            let ha = hl.alpha() * alpha;
            if ha > 0.01 {
                let hl_sz = tile_px * ((128.0 + PATH_HL_PAD as f32 * 2.0) / 128.0);
                for &(le, lx) in &hl.segs {
                    if let Some((tid, _, _)) = bank.path_highlight(le, lx) {
                        canvas.draw_tex_uv_rotated_blur(
                            tid,
                            uv_full,
                            cx,
                            cy,
                            hl_sz,
                            hl_sz,
                            tile.visual_rot_deg,
                            None,
                            1.0,
                            ha,
                            mirror_x,
                            blur,
                        );
                    }
                }
            }
        }

        if let Some((tid, _, _)) = bank.named(clear_key) {
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
            if is_sel {
                let b_alpha = (0.5 + 0.5 * (time * std::f32::consts::TAU / 1.6).sin()) * alpha;
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv_full,
                    cx,
                    cy,
                    tile_px,
                    tile_px,
                    tile.visual_rot_deg,
                    Some((tile.pipe_color, true)),
                    1.0,
                    b_alpha,
                    mirror_x,
                    blur,
                );
            }
        }

        if tile.is_freeze
            && let Some(frost_key) = frost_bitmap_key(tile.shape)
            && let Some((tid, _, _)) = bank.named(frost_key)
        {
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

        if tile.piece_type == PieceType::Source
            && let Some((tid, _, _)) = bank.named("PipeSpigotBitmap")
        {
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

        if color_blind {
            let badge = if tile.piece_type == PieceType::Portal {
                tile.secondary_color
                    .unwrap_or(GameColor::Violet)
                    .color_blind_char()
            } else {
                tile.pipe_color.color_blind_char()
            };
            let sc = tile_px / 128.0;
            fonts.draw_fx(
                canvas,
                FontFamily::Neuropol,
                cx,
                cy - 22.0 * sc,
                40.0,
                1.0,
                1.0,
                sc,
                sc,
                (255, 255, 255),
                Some((0x05, 0x0D, 0x2B)),
                TextAlign::Center,
                badge,
                1.0,
                alpha,
                [1.0; 3],
                blur,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_tile_filter_halo(
    canvas: &mut Canvas,
    bank: &TexBank,
    clear_key: &'static str,
    cx: f32,
    cy: f32,
    tile_px: f32,
    alpha: f32,
    blur: (f32, f32),
    rot_deg: f32,
    mirror_x: bool,
    is_sel: bool,
    is_tut: bool,
    is_hov_active: bool,
    is_elig: bool,
) {
    let uv_full = [0.0, 0.0, 1.0, 1.0];
    let rot = if clear_key == "CementBlockerBitmap" {
        0.0
    } else {
        rot_deg
    };
    let scale = tile_px / 128.0;
    if let Some((sh, sw, sh_h, _)) = bank.glow_named(clear_key, GlowPreset::PipeDropShadow) {
        canvas.draw_tex_uv_rotated_blur(
            sh,
            uv_full,
            cx,
            cy,
            sw as f32 * scale,
            sh_h as f32 * scale,
            rot,
            None,
            1.0,
            alpha,
            mirror_x,
            blur,
        );
    }
    let preset = if is_sel {
        Some(GlowPreset::PipeSelected)
    } else if is_tut {
        Some(GlowPreset::PipeTutorial)
    } else if is_hov_active {
        Some(GlowPreset::PipeHover)
    } else if is_elig {
        Some(GlowPreset::PipeAccent)
    } else {
        None
    };
    if let Some(p) = preset
        && let Some((glow, gw, gh, _)) = bank.glow_named(clear_key, p)
    {
        canvas.draw_tex_uv_rotated_blur(
            glow,
            uv_full,
            cx,
            cy,
            gw as f32 * scale,
            gh as f32 * scale,
            rot,
            None,
            1.0,
            alpha,
            mirror_x,
            blur,
        );
    }
}
