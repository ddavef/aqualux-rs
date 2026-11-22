// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

struct StageUniform {
    inv_half: vec2<f32>,
    _pad: vec2<f32>,
};

struct ColorMat {
    r0: vec4<f32>,
    r1: vec4<f32>,
    r2: vec4<f32>,
    off: vec4<f32>,
};

struct ColorMatrixTable {
    mats: array<ColorMat, 17>,
};

@group(0) @binding(0) var<uniform> stage: StageUniform;
@group(0) @binding(1) var<uniform> cmat: ColorMatrixTable;
@group(0) @binding(2) var s_lin: sampler;
@group(1) @binding(0) var t_tex: texture_2d<f32>;

struct PipeMaskIn {
    @location(0) row0: vec4<f32>,
    @location(1) row1: vec4<f32>,
    @location(2) cap0: vec4<f32>,
    @location(3) cap1: vec4<f32>,
    @location(4) cap2: vec4<f32>,
    @location(5) cap3: vec4<f32>,
    @location(6) radii: vec4<f32>,
    @location(7) params: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) tile_uv: vec2<f32>,
    @location(1) cap0: vec4<f32>,
    @location(2) cap1: vec4<f32>,
    @location(3) cap2: vec4<f32>,
    @location(4) cap3: vec4<f32>,
    @location(5) radii: vec4<f32>,
    @location(6) params: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, pm: PipeMaskIn) -> VsOut {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    let q = corners[vid];
    let lx = q.x * pm.row0.w;
    let ly = q.y * pm.row1.w;
    let px = pm.row0.x * lx + pm.row0.y * ly + pm.row0.z;
    let py = pm.row1.x * lx + pm.row1.y * ly + pm.row1.z;

    var out: VsOut;
    out.pos = vec4<f32>(
        px * stage.inv_half.x - 1.0,
        1.0 - py * stage.inv_half.y,
        0.0,
        1.0,
    );
    out.tile_uv = q;
    out.cap0 = pm.cap0;
    out.cap1 = pm.cap1;
    out.cap2 = pm.cap2;
    out.cap3 = pm.cap3;
    out.radii = pm.radii;
    out.params = pm.params;
    return out;
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let denom = max(dot(ba, ba), 1e-6);
    let h = clamp(dot(pa, ba) / denom, 0.0, 1.0);
    return length(pa - ba * h);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let tex = textureSample(t_tex, s_lin, in.tile_uv);
    let p = in.tile_uv * 128.0;
    let count = u32(in.params.z + 0.5);
    let caps = array<vec4<f32>, 4>(in.cap0, in.cap1, in.cap2, in.cap3);
    var min_d = 1e6;
    for (var i = 0u; i < 4u; i = i + 1u) {
        if i >= count {
            break;
        }
        let seg = caps[i];
        let rad = in.radii[i];
        let d = sd_segment(p, seg.xy, seg.zw) - rad;
        min_d = min(min_d, d);
    }
    let aa = max(fwidth(min_d), 0.05);
    let mask = clamp(0.5 - min_d / aa, 0.0, 1.0);
    if tex.a * mask <= 0.0001 {
        discard;
    }

    var rgb = tex.rgb;
    let midx = u32(in.params.x + 0.5);
    if midx > 0u && midx < 17u {
        let m = cmat.mats[midx];
        let nr = dot(m.r0.xyz, rgb) + m.off.x * tex.a;
        let ng = dot(m.r1.xyz, rgb) + m.off.y * tex.a;
        let nb = dot(m.r2.xyz, rgb) + m.off.z * tex.a;
        rgb = clamp(vec3<f32>(nr, ng, nb), vec3<f32>(0.0), vec3<f32>(tex.a));
    }
    let k = mask * in.params.y;
    return vec4<f32>(rgb * k, tex.a * k);
}
