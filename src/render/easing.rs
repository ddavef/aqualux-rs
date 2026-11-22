// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use std::f32::consts::PI;

pub fn quad_out(t: f32) -> f32 {
    let u = t.clamp(0.0, 1.0);
    1.0 - (1.0 - u) * (1.0 - u)
}

pub fn strong_out(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u * u * u
}

pub fn strong_in(t: f32) -> f32 {
    let u = t.clamp(0.0, 1.0);
    u * u * u * u * u
}

pub fn sine_out(t: f32) -> f32 {
    (t.clamp(0.0, 1.0) * (PI * 0.5)).sin()
}

pub fn back_out(t: f32) -> f32 {
    const S: f32 = 1.70158;
    let u = t.clamp(0.0, 1.0) - 1.0;
    u * u * ((S + 1.0) * u + S) + 1.0
}

pub fn back_in(t: f32) -> f32 {
    const S: f32 = 1.70158;
    let u = t.clamp(0.0, 1.0);
    u * u * ((S + 1.0) * u - S)
}

pub fn bounce_out(t: f32) -> f32 {
    let mut u = t.clamp(0.0, 1.0);
    if u < 1.0 / 2.75 {
        7.5625 * u * u
    } else if u < 2.0 / 2.75 {
        u -= 1.5 / 2.75;
        7.5625 * u * u + 0.75
    } else if u < 2.5 / 2.75 {
        u -= 2.25 / 2.75;
        7.5625 * u * u + 0.9375
    } else {
        u -= 2.625 / 2.75;
        7.5625 * u * u + 0.984375
    }
}

pub fn elastic_pulse(t: f32) -> f32 {
    let u = t.clamp(0.0, 1.0);
    if u == 0.0 || u == 1.0 {
        return 0.0;
    }
    (-(6.0 * u)).exp() * (u * PI * 4.0).sin()
}
