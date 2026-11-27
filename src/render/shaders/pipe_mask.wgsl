// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

struct StageUniform {
    inv_half: vec2<f32>,
    hdr_params: vec2<f32>,
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
    @location(7) blur_uv: vec2<f32>,
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
    let bx = pm.params.w;
    let lw = max(pm.row0.w, 1e-4);
    let lh = max(pm.row1.w, 1e-4);
    let pad = select(0.0, bx * 0.6, bx > 0.25);
    let lx = -pad + q.x * (pm.row0.w + 2.0 * pad);
    let ly = -pad + q.y * (pm.row1.w + 2.0 * pad);
    let px = pm.row0.x * lx + pm.row0.y * ly + pm.row0.z;
    let py = pm.row1.x * lx + pm.row1.y * ly + pm.row1.z;

    var out: VsOut;
    out.pos = vec4<f32>(
        px * stage.inv_half.x - 1.0,
        1.0 - py * stage.inv_half.y,
        0.0,
        1.0,
    );
    out.tile_uv = vec2<f32>(lx / lw, ly / lh);
    out.cap0 = pm.cap0;
    out.cap1 = pm.cap1;
    out.cap2 = pm.cap2;
    out.cap3 = pm.cap3;
    out.radii = pm.radii;
    out.params = pm.params;
    out.blur_uv = vec2<f32>(bx * 0.5 / lw, bx * 0.5 / lh);
    return out;
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let denom = max(dot(ba, ba), 1e-6);
    let h = clamp(dot(pa, ba) / denom, 0.0, 1.0);
    return length(pa - ba * h);
}

fn eval_pipe_masked(uv: vec2<f32>, in: VsOut, aa: f32) -> vec4<f32> {
    let inside = uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0;
    let tex = textureSampleLevel(t_tex, s_lin, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0);
    let p = uv * 128.0;
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
    let mask = select(0.0, clamp(0.5 - min_d / aa, 0.0, 1.0), inside);
    var rgb = tex.rgb;
    let midx = u32(in.params.x + 0.5);
    if midx > 0u && midx < 17u {
        let m = cmat.mats[midx];
        let nr = dot(m.r0.xyz, rgb) + m.off.x * tex.a;
        let ng = dot(m.r1.xyz, rgb) + m.off.y * tex.a;
        let nb = dot(m.r2.xyz, rgb) + m.off.z * tex.a;
        rgb = clamp(vec3<f32>(nr, ng, nb), vec3<f32>(0.0), vec3<f32>(tex.a));
    }
    if stage.hdr_params.x > 0.5 && tex.a > 0.001 {
        let straight = rgb / tex.a;
        let lum = dot(straight, vec3<f32>(0.2126, 0.7152, 0.0722));
        rgb *= mix(1.12, 1.32, smoothstep(0.35, 0.95, lum));
    }
    return vec4<f32>(rgb * mask, tex.a * mask);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let aa = max(max(fwidth(in.tile_uv.x), fwidth(in.tile_uv.y)) * 128.0, 0.05);
    let bx = in.params.w;
    var sample_rgba: vec4<f32>;
    if bx <= 0.25 {
        sample_rgba = eval_pipe_masked(in.tile_uv, in, aa);
    } else {
        var acc = vec4<f32>(0.0);
        var wsum = 0.0;
        let n = select(3, 4, bx > 20.0);
        let inv_n = 1.0 / f32(n);
        for (var j = -n; j <= n; j = j + 1) {
            let fy = f32(j) * inv_n;
            for (var i = -n; i <= n; i = i + 1) {
                let fx = f32(i) * inv_n;
                let w = exp(-(fx * fx + fy * fy) * 2.5);
                acc += eval_pipe_masked(in.tile_uv + vec2<f32>(fx * in.blur_uv.x, fy * in.blur_uv.y), in, aa) * w;
                wsum += w;
            }
        }
        sample_rgba = acc / max(wsum, 1e-5);
    }
    if sample_rgba.a * in.params.y <= 0.0001 {
        discard;
    }
    return sample_rgba * in.params.y;
}
