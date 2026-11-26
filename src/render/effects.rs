// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::render::canvas::{Canvas, LOGICAL_H, LOGICAL_W};
use crate::render::easing::{back_in, back_out, quad_out, sine_out, strong_in};
use crate::render::particles::FireworkBurst;
use crate::render::tex_bank::TexBank;
use crate::render::text::{FontFamily, FontSet, TextAlign, TextOutline};

#[derive(Debug, Clone)]
pub struct ScoreBlurbFx {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub age: f32,
}

impl ScoreBlurbFx {
    pub fn is_done(&self) -> bool {
        self.age >= 0.9
    }
}

/// Renders the 6-stripe Bezier rainbow arch (`(0,550) -> (200,220) -> (800,140)` with control point `(0,95)`).
pub fn draw_rainbow(canvas: &mut Canvas, bank: &TexBank, progress: f32, alpha: f32) {
    let t_end = strong_in(progress).clamp(0.01, 1.0);
    canvas.draw_rainbow_tex(bank.rainbow_tex(), t_end, alpha);
}

/// Renders tutorial bubble: rounded `0xEEEEEE` box, `4px` black stroke, pointer, `Helvetica` 18px text.
#[allow(clippy::too_many_arguments)]
pub fn draw_tutorial_bubble(
    canvas: &mut Canvas,
    fonts: &FontSet,
    target_cx: f32,
    target_cy: f32,
    text: &str,
    has_click_to_continue: bool,
    age: f32,
) -> (f32, f32, f32, f32) {
    let narrow =
        text.contains("whirl") || text.contains("FASTFORWARD") || text.contains("click here");
    let width = if narrow { 180.0 } else { 250.0 };
    let text_w = if narrow { 160.0 } else { 230.0 };
    let lines = fonts.wrap_lines(FontFamily::Helvetica, text, 18.0, text_w);
    let text_h =
        (lines.len().max(1) as f32) * 21.0 + if has_click_to_continue { 44.0 } else { 0.0 };
    let height = (text_h + 25.0).max(58.0);
    let tri_h = width / 4.0;
    let tri_half_w = (height / 10.0).max(8.0);

    let pop = if age < 0.2 {
        0.01
    } else {
        back_out(((age - 0.2) / 0.6).clamp(0.0, 1.0)).max(0.01)
    };
    let bw = width * pop;
    let bh = height * pop;
    let b_cy = (target_cy - height / 1.3 - tri_h)
        .clamp(bh * 0.5 + 10.0, LOGICAL_H as f32 - bh * 0.5 - 30.0);
    let b_cx = (target_cx - if text.contains("whirl") { 26.0 } else { 0.0 })
        .clamp(bw * 0.5 + 12.0, LOGICAL_W as f32 - bw * 0.5 - 12.0);
    let bx = b_cx - bw * 0.5;
    let by = b_cy - bh * 0.5;

    if pop > 0.05 {
        let r = (35.0 * pop).min(bw * 0.45).min(bh * 0.45);
        canvas.draw_tutorial_bubble(
            bx,
            by,
            bw,
            bh,
            r,
            4.0 * pop,
            bx + bw * 0.5,
            tri_half_w * pop,
            tri_h * pop,
        );
    }

    if pop > 0.75 {
        let mut cur_y = by + 13.0;
        for ln in &lines {
            fonts.draw(
                canvas,
                FontFamily::Helvetica,
                b_cx,
                cur_y,
                18.0,
                (0, 0, 0),
                TextAlign::Center,
                ln,
            );
            cur_y += 21.0;
        }
        if has_click_to_continue {
            fonts.draw(
                canvas,
                FontFamily::Helvetica,
                b_cx,
                by + bh - 30.0,
                16.0,
                (0x40, 0xA0, 0xFF),
                TextAlign::Center,
                "Click to continue",
            );
        }
    }
    (bx, by, bw, bh)
}

pub fn draw_fireworks(
    canvas: &mut Canvas,
    bank: &TexBank,
    fireworks: &[FireworkBurst],
    fw_alpha: f32,
) {
    for f in fireworks {
        f.draw(canvas, bank, fw_alpha);
    }
}

pub fn draw_score_blurbs(canvas: &mut Canvas, fonts: &FontSet, blurbs: &[ScoreBlurbFx]) {
    for b in blurbs {
        let (y_off, scale, alpha, col, glow_blur, glow_a, blur) = if b.age < 0.6 {
            let u = (b.age / 0.6).clamp(0.0, 1.0);
            let e = sine_out(u);
            let c_mix = quad_out(u);
            let r = (228.0 + (255.0 - 228.0) * c_mix) as u8;
            let g = (233.0 + (255.0 - 233.0) * c_mix) as u8;
            let bl = (237.0 + (255.0 - 237.0) * c_mix) as u8;
            let a = 1.0 - 0.5 * e;
            (
                -60.0 * e,
                1.0 + 0.3 * e,
                a,
                (r, g, bl),
                24.0 * e,
                0.55 * a,
                (0.0, 0.0),
            )
        } else {
            let u = ((b.age - 0.6) / 0.3).clamp(0.0, 1.0);
            let e = back_in(u).clamp(0.0, 1.0);
            (
                -60.0,
                (1.3 * (1.0 - e)).max(0.1),
                (0.5 * (1.0 - e)).clamp(0.0, 1.0),
                (255, 255, 255),
                0.0,
                0.0,
                (u * 16.0, u * 16.0),
            )
        };
        let base_fsz = 40.0;
        let y_pos = b.y + y_off - base_fsz * scale * 0.5;
        let tint_rgb = [
            col.0 as f32 / 255.0,
            col.1 as f32 / 255.0,
            col.2 as f32 / 255.0,
        ];
        if glow_a > 0.01 {
            fonts.draw_glow_alpha(
                canvas,
                FontFamily::Pricedown,
                b.x,
                y_pos,
                base_fsz,
                scale,
                TextAlign::Center,
                &b.text,
                (255, 255, 255),
                Some(TextOutline::stroke((0x05, 0x0D, 0x2B))),
                (255, 255, 255),
                glow_blur,
                glow_a,
            );
        }
        fonts.draw_fx(
            canvas,
            FontFamily::Pricedown,
            b.x,
            y_pos,
            base_fsz,
            1.0,
            1.0,
            scale,
            scale,
            (255, 255, 255),
            Some(TextOutline::stroke((0x05, 0x0D, 0x2B))),
            TextAlign::Center,
            &b.text,
            1.0,
            alpha,
            tint_rgb,
            blur,
        );
    }
}
