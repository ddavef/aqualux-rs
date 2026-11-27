// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::GameColor;
use crate::render::color_matrix::{IDENTITY, matrix_for_game_color, matrix_for_game_color_bright};
use bytemuck::{Pod, Zeroable};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TexId(pub u32);

pub fn color_mat_id(tint: Option<(GameColor, bool)>) -> u32 {
    let Some((color, bright)) = tint else {
        return 0;
    };
    let idx = match color {
        GameColor::Red => 0,
        GameColor::Orange => 1,
        GameColor::Yellow => 2,
        GameColor::Green => 3,
        GameColor::Blue => 4,
        GameColor::Violet => 5,
        GameColor::White => 6,
        GameColor::Grey => 7,
    };
    1 + idx + if bright { 8 } else { 0 }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct StageUniform {
    pub inv_half: [f32; 2],
    pub hdr_params: [f32; 2],
}

impl StageUniform {
    pub fn for_size(w: f32, h: f32) -> Self {
        Self {
            inv_half: [2.0 / w.max(1.0), 2.0 / h.max(1.0)],
            hdr_params: [0.0, 0.0],
        }
    }

    pub fn for_size_hdr(w: f32, h: f32) -> Self {
        Self {
            inv_half: [2.0 / w.max(1.0), 2.0 / h.max(1.0)],
            hdr_params: [1.0, 0.0],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct PresentUniform {
    pub params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ColorMatGpu {
    pub r0: [f32; 4],
    pub r1: [f32; 4],
    pub r2: [f32; 4],
    pub off: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ColorMatrixTable {
    pub mats: [ColorMatGpu; 17],
}

pub fn build_color_matrix_table() -> ColorMatrixTable {
    let to_gpu = |m: [f32; 20]| ColorMatGpu {
        r0: [m[0], m[1], m[2], 0.0],
        r1: [m[5], m[6], m[7], 0.0],
        r2: [m[10], m[11], m[12], 0.0],
        off: [m[4] / 255.0, m[9] / 255.0, m[14] / 255.0, 0.0],
    };
    let colors = [
        GameColor::Red,
        GameColor::Orange,
        GameColor::Yellow,
        GameColor::Green,
        GameColor::Blue,
        GameColor::Violet,
        GameColor::White,
        GameColor::Grey,
    ];
    let mut mats = [to_gpu(IDENTITY); 17];
    for (i, c) in colors.into_iter().enumerate() {
        mats[1 + i] = to_gpu(matrix_for_game_color(c));
        mats[9 + i] = to_gpu(matrix_for_game_color_bright(c, 1.8));
    }
    ColorMatrixTable { mats }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SpriteInstance {
    pub row0: [f32; 4],
    pub row1: [f32; 4],
    pub uv_rect: [f32; 4],
    pub params: [f32; 4],
    pub tint: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct PipeMaskInstance {
    pub row0: [f32; 4],
    pub row1: [f32; 4],
    pub cap0: [f32; 4],
    pub cap1: [f32; 4],
    pub cap2: [f32; 4],
    pub cap3: [f32; 4],
    pub radii: [f32; 4],
    pub params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ShapeInstance {
    pub bounds: [f32; 4],
    pub p0: [f32; 4],
    pub p1: [f32; 4],
    pub fill: [f32; 4],
    pub stroke: [f32; 4],
    pub meta: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct LeakUniform {
    pub bbox: [f32; 4],
    pub rgb_alpha: [f32; 4],
    pub count_pad: [u32; 4],
    pub pts: [[f32; 4]; 72],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct BlurUniform {
    pub color: [f32; 4],
    pub pass_info: [f32; 4],
    pub size_info: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GlyphUniform {
    pub inv_half: [f32; 4],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GlyphVertex {
    pub pos: [f32; 2],
}
