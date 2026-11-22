// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

struct GlyphUniform {
    inv_half: vec2<f32>,
    _pad: vec2<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: GlyphUniform;

@vertex
fn vs_main(@location(0) pos: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(
        pos.x * u.inv_half.x - 1.0,
        1.0 - pos.y * u.inv_half.y,
        0.0,
        1.0,
    );
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(u.color.rgb * u.color.a, u.color.a);
}
