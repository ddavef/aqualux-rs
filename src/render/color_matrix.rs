// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::GameColor;

const LUM_R: f32 = 0.212671;
const LUM_G: f32 = 0.71516;
const LUM_B: f32 = 0.072169;

pub type ColorMatrix = [f32; 20];

pub const IDENTITY: ColorMatrix = [
    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    0.0,
];

/// Composes two 4x5 RGBA color matrices with integer-truncated offset addition.
pub fn apply_matrix(a: &ColorMatrix, b: &ColorMatrix) -> ColorMatrix {
    let mut out = [0.0f32; 20];
    let mut row = 0usize;
    for _ in 0..4 {
        for col in 0..5 {
            let off = if col == 4 {
                (a[row + 4] as i32) as f32
            } else {
                0.0
            };
            out[row + col] = a[row] * b[col]
                + a[row + 1] * b[col + 5]
                + a[row + 2] * b[col + 10]
                + a[row + 3] * b[col + 15]
                + off;
        }
        row += 5;
    }
    out
}

pub fn set_brightness(m: ColorMatrix, n: f32) -> ColorMatrix {
    if n.is_nan() {
        return m;
    }
    let b = n * 100.0 - 100.0;
    let mat = [
        1.0, 0.0, 0.0, 0.0, b, 0.0, 1.0, 0.0, 0.0, b, 0.0, 0.0, 1.0, 0.0, b, 0.0, 0.0, 0.0, 1.0,
        0.0,
    ];
    apply_matrix(&mat, &m)
}

pub fn set_hue(m: ColorMatrix, deg: f32) -> ColorMatrix {
    if deg.is_nan() {
        return m;
    }
    let rad = deg.to_radians();
    let c = rad.cos();
    let s = rad.sin();
    let mat = [
        LUM_R + c * (1.0 - LUM_R) + s * (-LUM_R),
        LUM_G + c * (-LUM_G) + s * (-LUM_G),
        LUM_B + c * (-LUM_B) + s * (1.0 - LUM_B),
        0.0,
        0.0,
        LUM_R + c * (-LUM_R) + s * 0.143,
        LUM_G + c * (1.0 - LUM_G) + s * 0.14,
        LUM_B + c * (-LUM_B) + s * (-0.283),
        0.0,
        0.0,
        LUM_R + c * (-LUM_R) + s * (-(1.0 - LUM_R)),
        LUM_G + c * (-LUM_G) + s * LUM_G,
        LUM_B + c * (1.0 - LUM_B) + s * LUM_B,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ];
    apply_matrix(&mat, &m)
}

pub fn set_saturation(m: ColorMatrix, s: f32) -> ColorMatrix {
    if s.is_nan() {
        return m;
    }
    let inv = 1.0 - s;
    let ir = inv * LUM_R;
    let ig = inv * LUM_G;
    let ib = inv * LUM_B;
    let mat = [
        ir + s,
        ig,
        ib,
        0.0,
        0.0,
        ir,
        ig + s,
        ib,
        0.0,
        0.0,
        ir,
        ig,
        ib + s,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ];
    apply_matrix(&mat, &m)
}

pub fn matrix_for_game_color(color: GameColor) -> ColorMatrix {
    let (hue, brightness, saturation) = color.matrix_props();
    let m = set_brightness(IDENTITY, brightness);
    let m = set_hue(m, hue);
    set_saturation(m, saturation)
}

pub fn matrix_for_game_color_bright(color: GameColor, brightness_mul: f32) -> ColorMatrix {
    set_brightness(matrix_for_game_color(color), brightness_mul)
}
