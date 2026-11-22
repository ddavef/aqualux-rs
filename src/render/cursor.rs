// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::render::canvas::{Canvas, Rect};
use crate::render::tex_bank::TexBank;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CursorKind {
    Arrow,
    Move,
    Rotate,
    Stop,
    Wait,
}

impl CursorKind {
    pub fn bitmap_name(self) -> &'static str {
        match self {
            Self::Arrow => "ArrowCursorBitmap",
            Self::Move => "MoveCursorBitmap",
            Self::Rotate => "RotateCursorBitmap",
            Self::Stop => "StopCursorBitmap",
            Self::Wait => "WaitCursorBitmap",
        }
    }

    /// Mouse pointer hotspot offsets relative to each cursor graphic.
    pub fn offset(self) -> (f32, f32) {
        match self {
            Self::Arrow => (-3.0, -3.0),
            Self::Move => (-30.0, -30.0),
            Self::Rotate => (-20.0, -20.0),
            Self::Stop => (-27.0, -27.0),
            Self::Wait => (-27.0, -27.0),
        }
    }
}

/// Renders the custom cursor bitmap with pre-baked drop shadow (`blur=16`).
pub fn draw_cursor(canvas: &mut Canvas, bank: &TexBank, kind: CursorKind, mx: f32, my: f32) {
    let Some((tid, w, h, pad)) = bank.cursor(kind) else {
        return;
    };
    let (ox, oy) = kind.offset();
    canvas.draw_tex_rect(
        tid,
        Rect::new(
            mx + ox - pad as f32,
            my + oy - pad as f32,
            w as f32,
            h as f32,
        ),
        None,
        1.0,
        1.0,
    );
}
