// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

struct PresentUniform {
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: PresentUniform;
@group(0) @binding(1) var t_scene: texture_2d<f32>;
@group(0) @binding(2) var t_bloom: texture_2d<f32>;
@group(0) @binding(3) var s_lin: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    let corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(corners[vid], 0.0, 1.0);
}

fn ext_srgb_to_linear_ch(x: f32) -> f32 {
    if x <= 0.04045 {
        return x / 12.92;
    }
    return pow((x + 0.055) / 1.055, 2.4);
}

fn ext_srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        ext_srgb_to_linear_ch(c.r),
        ext_srgb_to_linear_ch(c.g),
        ext_srgb_to_linear_ch(c.b),
    );
}

fn rolloff_hdr(lin: vec3<f32>, headroom: f32) -> vec3<f32> {
    let peak = max(lin.r, max(lin.g, lin.b));
    if peak <= 1.0 {
        return lin;
    }
    if headroom <= 1.001 {
        return lin / peak;
    }
    let span = headroom - 1.0;
    let excess = peak - 1.0;
    let mapped = 1.0 + span * (excess / (excess + span));
    return lin * (mapped / peak);
}

fn p3_to_bt709(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        1.2249401 * c.r - 0.2249401 * c.g,
        -0.0420569 * c.r + 1.0420569 * c.g,
        -0.0196376 * c.r - 0.0786361 * c.g + 1.0982737 * c.b,
    );
}

fn bt709_to_bt2020(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        0.6274039 * c.r + 0.3292830 * c.g + 0.0433131 * c.b,
        0.0690973 * c.r + 0.9195404 * c.g + 0.0113623 * c.b,
        0.0163914 * c.r + 0.0880133 * c.g + 0.8955953 * c.b,
    );
}

fn linear_to_pq(y_in: vec3<f32>) -> vec3<f32> {
    let m1 = 2610.0 / 16384.0;
    let m2 = 2523.0 / 32.0;
    let c1 = 3424.0 / 4096.0;
    let c2 = 2413.0 / 128.0;
    let c3 = 2392.0 / 128.0;
    let y = clamp(y_in, vec3<f32>(0.0), vec3<f32>(1.0));
    let ym = pow(max(y, vec3<f32>(1e-7)), vec3<f32>(m1));
    let pq = pow((vec3<f32>(c1) + c2 * ym) / (vec3<f32>(1.0) + c3 * ym), vec3<f32>(m2));
    return select(vec3<f32>(0.0), pq, y > vec3<f32>(0.0));
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<i32>(textureDimensions(t_scene, 0));
    let coord = clamp(vec2<i32>(pos.xy), vec2<i32>(0), max(dims - vec2<i32>(1), vec2<i32>(0)));
    let texel = textureLoad(t_scene, coord, 0);
    let c_srgb = max(texel.rgb, vec3<f32>(0.0));
    let scene_lin = ext_srgb_to_linear(c_srgb);
    let uv = pos.xy / vec2<f32>(max(dims, vec2<i32>(1)));
    let bloom = max(textureSampleLevel(t_bloom, s_lin, uv, 0.0).rgb, vec3<f32>(0.0));
    var lin = scene_lin + bloom * 0.28;
    let lin_peak = max(lin.r, max(lin.g, lin.b));
    if lin_peak > 1.0 {
        let wcg_w = 0.55 * smoothstep(1.0, 1.8, lin_peak);
        lin = mix(lin, p3_to_bt709(lin), wcg_w);
    }
    let mapped = rolloff_hdr(lin, u.params.z);

    if u.params.w < 0.5 {
        return vec4<f32>(mapped * u.params.x, 1.0);
    }

    let bt2020 = max(bt709_to_bt2020(mapped), vec3<f32>(0.0));
    let pq = linear_to_pq(bt2020 * (u.params.y / 10000.0));
    return vec4<f32>(pq, 1.0);
}
