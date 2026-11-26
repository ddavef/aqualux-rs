// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::render::easing::quad_out;
use crate::render::{Canvas, LOGICAL_H, LOGICAL_W, Rect, TexBank};

pub const SPLASH_HIDE_START: f32 = 3.8;
pub const SPLASH_END: f32 = 4.2;

/// Fades `MmgSplashBitmap` in (`0..0.8s`) over a white stage panel, then fades the whole splash container out (`3.8..4.2s`).
pub fn render_splash_overlay(canvas: &mut Canvas, bank: &TexBank, age: f32) {
    let container_alpha = if age < SPLASH_HIDE_START {
        1.0
    } else {
        (1.0 - quad_out(((age - SPLASH_HIDE_START) / 0.4).clamp(0.0, 1.0))).clamp(0.0, 1.0)
    };
    if container_alpha <= 0.004 {
        return;
    }
    let bmp_alpha = quad_out((age / 0.8).clamp(0.0, 1.0));
    canvas.fill_rect_alpha(
        Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
        255,
        255,
        255,
        (container_alpha * 255.0).round() as u8,
    );
    let total_bmp_alpha = bmp_alpha * container_alpha;
    if total_bmp_alpha > 0.004
        && let Some((tid, bw, bh)) = bank.named("MmgSplashBitmap")
    {
        let w = bw as f32;
        let h = bh as f32;
        let x = (LOGICAL_W as f32 - w) * 0.5;
        let y = (LOGICAL_H as f32 - h) * 0.5;
        canvas.draw_tex_rect(tid, Rect::new(x, y, w, h), None, 1.0, total_bmp_alpha);
    }
}

/// Renders the standalone splash screen over black stage background before transitioning to Title.
pub fn render_splash(canvas: &mut Canvas, bank: &TexBank, age: f32) {
    canvas.clear((0, 0, 0));
    render_splash_overlay(canvas, bank, age);
}
