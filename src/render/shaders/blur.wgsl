// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

struct BlurUniform {
    color: vec4<f32>,
    pass_info: vec4<f32>,
    size_info: vec4<f32>,
};

@group(0) @binding(0) var<uniform> b: BlurUniform;
@group(0) @binding(1) var t_src: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    let pts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, -3.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pts[vid], 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dir_x = i32(round(b.pass_info.x));
    let dir_y = i32(round(b.pass_info.y));
    let sigma = max(b.pass_info.z, 0.5);
    let gain = b.pass_info.w;

    let src_w = i32(round(b.size_info.x));
    let src_h = i32(round(b.size_info.y));
    let shift_x = i32(round(b.size_info.z));
    let shift_y = i32(round(b.size_info.w));

    let rad = min(i32(ceil(3.0 * sigma)), 48);
    let dst_xy = vec2<i32>(i32(in.pos.x), i32(in.pos.y));
    let inv2s2 = 0.5 / (sigma * sigma);

    var w_sum = 0.0;
    var a_sum = 0.0;
    for (var k = -rad; k <= rad; k = k + 1) {
        let w = exp(-f32(k * k) * inv2s2);
        w_sum += w;
        let sx = dst_xy.x + k * dir_x - shift_x;
        let sy = dst_xy.y + k * dir_y - shift_y;
        if sx >= 0 && sx < src_w && sy >= 0 && sy < src_h {
            a_sum += w * textureLoad(t_src, vec2<i32>(sx, sy), 0).a;
        }
    }

    let a_norm = a_sum / max(w_sum, 1e-6);
    if dir_x != 0 {
        return vec4<f32>(0.0, 0.0, 0.0, a_norm);
    }
    let final_a = clamp(a_norm * gain, 0.0, 1.0);
    return vec4<f32>(b.color.rgb * final_a, final_a);
}
