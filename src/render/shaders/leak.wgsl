// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

struct StageUniform {
    inv_half: vec2<f32>,
    hdr_params: vec2<f32>,
};

struct LeakUniform {
    bbox: vec4<f32>,
    rgb_alpha: vec4<f32>,
    count_pad: vec4<u32>,
    pts: array<vec4<f32>, 72>,
};

@group(0) @binding(0) var<uniform> stage: StageUniform;
@group(1) @binding(0) var<uniform> leak: LeakUniform;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) pt: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    let q = corners[vid];
    let pt = leak.bbox.xy + q * leak.bbox.zw;
    var out: VsOut;
    out.pos = vec4<f32>(
        pt.x * stage.inv_half.x - 1.0,
        1.0 - pt.y * stage.inv_half.y,
        0.0,
        1.0,
    );
    out.pt = pt;
    return out;
}

fn get_pt(idx: u32) -> vec2<f32> {
    let v = leak.pts[idx >> 1u];
    return select(v.xy, v.zw, (idx & 1u) == 1u);
}

fn sd_poly(p: vec2<f32>, count: u32) -> f32 {
    var min_d2 = 1e12;
    var a = get_pt(0u);
    for (var i = 1u; i < count; i = i + 1u) {
        let b = get_pt(i);
        let pa = p - a;
        let ba = b - a;
        let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
        let d = pa - ba * h;
        min_d2 = min(min_d2, dot(d, d));
        a = b;
    }
    return sqrt(min_d2) - 19.0;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = leak.count_pad.x;
    if n < 2u {
        discard;
    }
    let p = in.pt;
    let d = sd_poly(p, n);
    let blur = bitcast<f32>(leak.count_pad.y);
    let aa = max(fwidth(d), 0.05);
    let edge_w = max(aa, blur * 1.15);
    let cov = clamp(0.5 - d / edge_w, 0.0, 1.0);
    if cov <= 0.001 {
        discard;
    }

    let off = vec2<f32>(11.313708, 11.313708);
    let bevel_w = max(7.2, blur * 1.15);
    let a1 = clamp(0.5 - sd_poly(p - off, n) / bevel_w, 0.0, 1.0);
    let a2 = clamp(0.5 - sd_poly(p + off, n) / bevel_w, 0.0, 1.0);
    let diff = a2 - a1;
    var rgb = leak.rgb_alpha.rgb;
    if diff > 0.0 {
        rgb = mix(rgb, vec3<f32>(1.0, 1.0, 179.0 / 255.0), clamp(diff * 0.4, 0.0, 1.0));
    } else if diff < 0.0 {
        rgb = mix(rgb, vec3<f32>(44.0 / 255.0, 51.0 / 255.0, 75.0 / 255.0), clamp(-diff * 0.8, 0.0, 1.0));
    }

    let glow_w = max(5.4, blur * 1.15);
    let g_blur = clamp(0.5 - d / glow_w, 0.0, 1.0);
    let edge = clamp((1.0 - g_blur) * 0.6, 0.0, 1.0);
    rgb = mix(rgb, vec3<f32>(1.0), edge);
    if stage.hdr_params.x > 0.5 {
        let spec = clamp(max(diff, 0.0) * 0.5 + edge * 0.85, 0.0, 1.0);
        rgb *= mix(1.12, 1.38, spec);
    }
    let a = cov * leak.rgb_alpha.a / (1.0 + blur * 0.025);
    return vec4<f32>(rgb * a, a);
}
