// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::{FillPass, FlowTimerState, Side};
use crate::render::canvas::{Canvas, Rect};
use crate::render::easing::{bounce_out, elastic_in_out, quad_out, strong_in};
use crate::render::tex_bank::{GlowPreset, TexBank};
use crate::render::text::{FontFamily, FontSet, TextAlign};

pub fn build_pipe_capsules(fills: &[FillPass], tile_dir: u8) -> ([[f32; 4]; 4], [f32; 4], u32) {
    let rot_back = (4 - (tile_dir % 4)) % 4;
    let side_pt = |s: Side| -> (f32, f32) {
        match s {
            Side::Up => (64.0, 0.0),
            Side::Right => (128.0, 64.0),
            Side::Down => (64.0, 128.0),
            Side::Left => (0.0, 64.0),
            Side::Center => (64.0, 64.0),
        }
    };
    let mut caps = [[0.0; 4]; 4];
    let mut radii = [0.0; 4];
    let mut count = 0usize;
    let mut push_cap = |x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
        if count < 4 {
            caps[count] = [x0, y0, x1, y1];
            radii[count] = r;
            count += 1;
        }
    };

    for pass in fills {
        let p = pass.progress.clamp(0.0, 1.0);
        if p <= 0.001 {
            continue;
        }
        let le = pass.entry.rotate_cw(rot_back);
        let lx = pass.exit.rotate_cw(rot_back);
        let a = side_pt(le);
        let b = side_pt(lx);
        if le == Side::Center {
            let t = ((p - 0.5).max(0.0) * 2.0).clamp(0.0, 1.0);
            let r = (1.0 + 47.0 * quad_out(pass.elapsed.clamp(0.0, 1.0))) * 0.5;
            push_cap(
                64.0,
                64.0,
                64.0 + (b.0 - 64.0) * t,
                64.0 + (b.1 - 64.0) * t,
                r,
            );
        } else if lx == Side::Center {
            let t = (p * 2.0).clamp(0.0, 1.2);
            push_cap(
                a.0,
                a.1,
                a.0 + (64.0 - a.0) * t,
                a.1 + (64.0 - a.1) * t,
                24.0,
            );
        } else if p < 0.5 {
            let t = p * 2.0;
            push_cap(
                a.0,
                a.1,
                a.0 + (64.0 - a.0) * t,
                a.1 + (64.0 - a.1) * t,
                24.0,
            );
        } else if (le as u8 + 2) % 4 == lx as u8 {
            let t = (p - 0.5) * 2.0;
            push_cap(
                a.0,
                a.1,
                64.0 + (b.0 - 64.0) * t,
                64.0 + (b.1 - 64.0) * t,
                24.0,
            );
        } else {
            let t = (p - 0.5) * 2.0;
            push_cap(a.0, a.1, 64.0, 64.0, 24.0);
            push_cap(
                64.0,
                64.0,
                64.0 + (b.0 - 64.0) * t,
                64.0 + (b.1 - 64.0) * t,
                24.0,
            );
        }
    }
    (caps, radii, count as u32)
}

#[allow(clippy::too_many_arguments)]
pub fn draw_flow_timer(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    cx: f32,
    cy: f32,
    timer: &FlowTimerState,
    alpha: f32,
) {
    let fade_a = if timer.fading {
        1.0 - quad_out(timer.fade_elapsed.clamp(0.0, 1.0))
    } else {
        1.0
    };
    let eff_alpha = alpha * fade_a;
    if eff_alpha <= 0.004 {
        return;
    }
    let pop_scale = if timer.age < 0.9 {
        0.01 + 0.99 * bounce_out((timer.age / 0.9).clamp(0.0, 1.0))
    } else {
        1.0
    };
    let secs = timer.remaining;
    let s = secs.ceil().max(0.0) as i32;
    let pulse = match timer.warn_elapsed {
        None => 1.0,
        Some(w) if w < 0.9 => 1.0 + 0.2 * elastic_in_out(w),
        Some(w) if w < 1.9 => {
            let s0 = 1.0 + 0.2 * elastic_in_out(0.9);
            s0 + (0.9 - s0) * elastic_in_out(w - 0.9)
        }
        Some(_) => 0.9,
    };
    let sc = (pop_scale * pulse).max(0.01);
    let bl = if timer.fading {
        strong_in(timer.fade_elapsed.clamp(0.0, 1.0)) * 64.0
    } else if timer.age < 0.9 {
        (1.0 - timer.age / 0.9) * 4.0
    } else {
        0.0
    };
    if let Some((gid, gw, gh, _)) = bank.glow_named("FlowTimerBitmap", GlowPreset::FlowTimerYellow)
    {
        let w = gw as f32 * sc;
        let h = gh as f32 * sc;
        canvas.draw_tex_rect_blur(
            gid,
            Rect::new(cx - w * 0.5, cy - h * 0.5, w, h),
            None,
            1.0,
            eff_alpha,
            (bl, bl),
        );
        canvas.boost_last_sprite(1.35);
    }
    if let Some((tid, tw, th)) = bank.named("FlowTimerBitmap") {
        let w = tw as f32 * sc;
        let h = th as f32 * sc;
        canvas.draw_tex_rect_blur(
            tid,
            Rect::new(cx - w * 0.5, cy - h * 0.5, w, h),
            None,
            1.0,
            eff_alpha,
            (bl, bl),
        );
    }
    if sc < 0.08 {
        return;
    }
    let label = format!("{}:{:02}", s / 60, s % 60);
    let col = if timer.warning_red {
        (255, 0, 0)
    } else {
        (0, 0, 0)
    };
    let y_off = 38.0 * sc * 0.55 - 6.0 * sc;
    fonts.draw_digits_fx(
        canvas,
        FontFamily::Pricedown,
        cx,
        cy - y_off,
        38.0,
        0.8,
        1.2,
        sc,
        col,
        None,
        TextAlign::Center,
        &label,
        eff_alpha,
        (bl, bl),
    );
}
