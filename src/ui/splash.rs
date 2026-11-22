// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::render::{Canvas, LOGICAL_H, LOGICAL_W, Rect, TexBank};

/// Solid white backdrop fading `MmgSplashBitmap` in (`0..0.8s`) and out (`3.8..4.2s`).
pub fn render_splash(canvas: &mut Canvas, bank: &TexBank, age: f32) {
    canvas.clear((255, 255, 255));
    let alpha = if age < 0.8 {
        (age / 0.8).clamp(0.0, 1.0)
    } else if age < 3.8 {
        1.0
    } else {
        (1.0 - (age - 3.8) / 0.4).clamp(0.0, 1.0)
    };
    if let Some((tid, bw, bh)) = bank.named("MmgSplashBitmap") {
        let w = bw as f32;
        let h = bh as f32;
        let x = (LOGICAL_W as f32 - w) * 0.5;
        let y = (LOGICAL_H as f32 - h) * 0.5;
        canvas.draw_tex_rect(tid, Rect::new(x, y, w, h), None, 1.0, alpha);
    }
}
