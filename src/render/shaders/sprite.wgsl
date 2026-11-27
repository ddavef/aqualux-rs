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

struct SpriteIn {
    @location(0) row0: vec4<f32>,
    @location(1) row1: vec4<f32>,
    @location(2) uv_rect: vec4<f32>,
    @location(3) params: vec4<f32>,
    @location(4) tint: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) params: vec4<f32>,
    @location(2) tint: vec4<f32>,
    @location(3) blur_uv: vec2<f32>,
    @location(4) uv_lim: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: SpriteIn) -> VsOut {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    let q = corners[vid];
    let bx = inst.params.w;
    let by = inst.tint.w;
    let lw = max(inst.row0.w, 1e-4);
    let lh = max(inst.row1.w, 1e-4);
    let pad_x = select(0.0, bx * 0.6, bx > 0.25);
    let pad_y = select(0.0, by * 0.6, by > 0.25);
    let lx = -pad_x + q.x * (inst.row0.w + 2.0 * pad_x);
    let ly = -pad_y + q.y * (inst.row1.w + 2.0 * pad_y);
    let px = inst.row0.x * lx + inst.row0.y * ly + inst.row0.z;
    let py = inst.row1.x * lx + inst.row1.y * ly + inst.row1.z;

    var out: VsOut;
    out.pos = vec4<f32>(
        px * stage.inv_half.x - 1.0,
        1.0 - py * stage.inv_half.y,
        0.0,
        1.0,
    );
    out.uv = inst.uv_rect.xy + vec2<f32>(lx / lw, ly / lh) * inst.uv_rect.zw;
    out.params = inst.params;
    out.tint = inst.tint;
    out.blur_uv = vec2<f32>(bx, by) * 0.5 / vec2<f32>(lw, lh) * inst.uv_rect.zw;
    out.uv_lim = vec4<f32>(inst.uv_rect.xy, inst.uv_rect.xy + inst.uv_rect.zw);
    return out;
}

fn sample_bounded(uv: vec2<f32>, lim: vec4<f32>) -> vec4<f32> {
    let c = textureSampleLevel(t_tex, s_lin, clamp(uv, lim.xy, lim.zw), 0.0);
    let inside = uv.x >= lim.x && uv.x <= lim.z && uv.y >= lim.y && uv.y <= lim.w;
    return select(vec4<f32>(0.0), c, inside);
}

fn sample_sprite(uv: vec2<f32>, bx: f32, by: f32, half_r: vec2<f32>, lim: vec4<f32>) -> vec4<f32> {
    if bx <= 0.25 && by <= 0.25 {
        return textureSampleLevel(t_tex, s_lin, uv, 0.0);
    }
    var acc = vec4<f32>(0.0);
    var wsum = 0.0;
    if by <= 0.25 {
        for (var k = -8; k <= 8; k = k + 1) {
            let fk = f32(k) * 0.125;
            let w = exp(-fk * fk * 2.5);
            acc += sample_bounded(uv + vec2<f32>(fk * half_r.x, 0.0), lim) * w;
            wsum += w;
        }
        return acc / max(wsum, 1e-5);
    }
    if bx <= 0.25 {
        for (var k = -8; k <= 8; k = k + 1) {
            let fk = f32(k) * 0.125;
            let w = exp(-fk * fk * 2.5);
            acc += sample_bounded(uv + vec2<f32>(0.0, fk * half_r.y), lim) * w;
            wsum += w;
        }
        return acc / max(wsum, 1e-5);
    }
    let n = select(3, 4, max(bx, by) > 20.0);
    let inv_n = 1.0 / f32(n);
    for (var j = -n; j <= n; j = j + 1) {
        let fy = f32(j) * inv_n;
        for (var i = -n; i <= n; i = i + 1) {
            let fx = f32(i) * inv_n;
            let w = exp(-(fx * fx + fy * fy) * 2.5);
            acc += sample_bounded(uv + vec2<f32>(fx * half_r.x, fy * half_r.y), lim) * w;
            wsum += w;
        }
    }
    return acc / max(wsum, 1e-5);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let tex = sample_sprite(in.uv, in.params.w, in.tint.w, in.blur_uv, in.uv_lim);
    if tex.a <= 0.0001 {
        discard;
    }
    let hdr_on = stage.hdr_params.x > 0.5;
    if in.params.y < -0.5 {
        let ga = min(tex.a * 1.6, 1.0);
        let a = in.params.z;
        let glow_boost = select(1.0, 1.32, hdr_on);
        return vec4<f32>(in.tint.rgb * glow_boost * ga * a, ga * a);
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
    let tint_rgb = select(min(in.tint.rgb, vec3<f32>(1.0)), in.tint.rgb, hdr_on);
    if in.params.y > 1.0 {
        let add = ((in.params.y - 1.0) * 100.0 / 255.0) * tex.a;
        let raw = rgb * tint_rgb + vec3<f32>(add);
        rgb = select(clamp(raw, vec3<f32>(0.0), vec3<f32>(tex.a)), max(raw, vec3<f32>(0.0)), hdr_on);
    } else {
        let raw = rgb * tint_rgb * in.params.y;
        rgb = select(clamp(raw, vec3<f32>(0.0), vec3<f32>(tex.a)), max(raw, vec3<f32>(0.0)), hdr_on);
    }
    let a = in.params.z;
    return vec4<f32>(rgb * a, tex.a * a);
}
