// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

struct BloomUniform {
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: BloomUniform;
@group(0) @binding(1) var t_src: texture_2d<f32>;
@group(0) @binding(2) var s_lin: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    let corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let ndc = corners[vid];
    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = vec2<f32>((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
    return out;
}

fn ext_srgb_to_linear_ch(x: f32) -> f32 {
    if x <= 0.04045 {
        return x / 12.92;
    }
    return pow((x + 0.055) / 1.055, 2.4);
}

fn extract_hdr_tap(c_in: vec3<f32>) -> vec3<f32> {
    let c = max(c_in, vec3<f32>(0.0));
    let lin = vec3<f32>(
        ext_srgb_to_linear_ch(c.r),
        ext_srgb_to_linear_ch(c.g),
        ext_srgb_to_linear_ch(c.b),
    );
    let peak = max(lin.r, max(lin.g, lin.b));
    if peak <= 1.0 {
        return vec3<f32>(0.0);
    }
    let excess = peak - 1.0;
    let contrib = (excess * excess) / (excess + 0.35);
    return lin * (contrib / peak);
}

@fragment
fn fs_extract(in: VsOut) -> @location(0) vec4<f32> {
    let h = u.params.xy * 0.5;
    let t0 = extract_hdr_tap(textureSampleLevel(t_src, s_lin, in.uv + vec2<f32>(-h.x, -h.y), 0.0).rgb);
    let t1 = extract_hdr_tap(textureSampleLevel(t_src, s_lin, in.uv + vec2<f32>(h.x, -h.y), 0.0).rgb);
    let t2 = extract_hdr_tap(textureSampleLevel(t_src, s_lin, in.uv + vec2<f32>(-h.x, h.y), 0.0).rgb);
    let t3 = extract_hdr_tap(textureSampleLevel(t_src, s_lin, in.uv + vec2<f32>(h.x, h.y), 0.0).rgb);
    return vec4<f32>((t0 + t1 + t2 + t3) * 0.25, 1.0);
}

@fragment
fn fs_blur(in: VsOut) -> @location(0) vec4<f32> {
    let step = u.params.zw * u.params.xy;
    let offsets = array<f32, 5>(0.0, 1.418, 3.304, 5.202, 7.108);
    let weights = array<f32, 5>(0.13298, 0.23228, 0.13534, 0.05110, 0.01479);
    var acc = textureSampleLevel(t_src, s_lin, in.uv, 0.0).rgb * weights[0];
    for (var i = 1u; i < 5u; i = i + 1u) {
        let off = step * offsets[i];
        let a = textureSampleLevel(t_src, s_lin, in.uv + off, 0.0).rgb;
        let b = textureSampleLevel(t_src, s_lin, in.uv - off, 0.0).rgb;
        acc += (a + b) * weights[i];
    }
    return vec4<f32>(acc, 1.0);
}
