// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

struct StageUniform {
    inv_half: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> stage: StageUniform;

struct ShapeIn {
    @location(0) bounds: vec4<f32>,
    @location(1) p0: vec4<f32>,
    @location(2) p1: vec4<f32>,
    @location(3) fill: vec4<f32>,
    @location(4) stroke: vec4<f32>,
    @location(5) shape_meta: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) pt: vec2<f32>,
    @location(1) p0: vec4<f32>,
    @location(2) p1: vec4<f32>,
    @location(3) fill: vec4<f32>,
    @location(4) stroke: vec4<f32>,
    @location(5) shape_meta: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: ShapeIn) -> VsOut {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    let q = corners[vid];
    let pt = inst.bounds.xy + q * inst.bounds.zw;

    var out: VsOut;
    out.pos = vec4<f32>(
        pt.x * stage.inv_half.x - 1.0,
        1.0 - pt.y * stage.inv_half.y,
        0.0,
        1.0,
    );
    out.pt = pt;
    out.p0 = inst.p0;
    out.p1 = inst.p1;
    out.fill = inst.fill;
    out.stroke = inst.stroke;
    out.shape_meta = inst.shape_meta;
    return out;
}

fn premul(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(c.rgb * c.a, c.a);
}

fn over_pm(fg: vec4<f32>, bg: vec4<f32>) -> vec4<f32> {
    return fg + bg * (1.0 - fg.a);
}

fn sd_rounded_box(p: vec2<f32>, half_ext: vec2<f32>, r: f32) -> f32 {
    let rr = min(r, min(half_ext.x, half_ext.y));
    let q = abs(p) - half_ext + vec2<f32>(rr);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - rr;
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h);
}

fn sd_triangle(p: vec2<f32>, p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>) -> f32 {
    let e0 = p1 - p0; let e1 = p2 - p1; let e2 = p0 - p2;
    let v0 = p - p0; let v1 = p - p1; let v2 = p - p2;
    let pq0 = v0 - e0 * clamp(dot(v0, e0) / max(dot(e0, e0), 1e-6), 0.0, 1.0);
    let pq1 = v1 - e1 * clamp(dot(v1, e1) / max(dot(e1, e1), 1e-6), 0.0, 1.0);
    let pq2 = v2 - e2 * clamp(dot(v2, e2) / max(dot(e2, e2), 1e-6), 0.0, 1.0);
    let s = sign(e0.x * e2.y - e0.y * e2.x);
    let d = min(
        min(
            vec2<f32>(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x)),
            vec2<f32>(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x)),
        ),
        vec2<f32>(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x)),
    );
    return -sqrt(d.x) * sign(d.y);
}

fn sd_tut_bubble(
    p: vec2<f32>,
    ctr: vec2<f32>,
    half_ext: vec2<f32>,
    radius: f32,
    v0: vec2<f32>,
    v1: vec2<f32>,
    v2: vec2<f32>,
) -> f32 {
    return min(sd_rounded_box(p - ctr, half_ext, radius), sd_triangle(p, v0, v1, v2));
}

fn eval_rainbow(p: vec2<f32>, t_end: f32, alpha: f32, aa: f32) -> vec4<f32> {
    let colors = array<vec3<f32>, 6>(
        vec3<f32>(1.0, 0.0, 0.0),
        vec3<f32>(1.0, 0.545098, 0.32549),
        vec3<f32>(1.0, 1.0, 0.0),
        vec3<f32>(0.50196, 1.0, 0.0),
        vec3<f32>(0.0, 0.50196, 1.0),
        vec3<f32>(1.0, 0.50196, 1.0),
    );
    var min_ds = array<f32, 6>(1e6, 1e6, 1e6, 1e6, 1e6, 1e6);
    var prev = vec2<f32>(0.0, 550.0);
    for (var s = 1u; s <= 28u; s = s + 1u) {
        let t = t_end * (f32(s) / 28.0);
        let u = 1.0 - t;
        let cur = vec2<f32>(t * t * 800.0, u * u * 550.0 + 2.0 * u * t * 95.0 + t * t * 140.0);
        for (var i = 0u; i < 6u; i = i + 1u) {
            let oy = f32(i) * 11.0;
            let d = sd_segment(p, vec2<f32>(prev.x, prev.y + oy), vec2<f32>(cur.x, cur.y + oy)) - 5.5;
            min_ds[i] = min(min_ds[i], d);
        }
        prev = cur;
    }
    var acc = vec4<f32>(0.0);
    let band_a = alpha;
    for (var i = 0u; i < 6u; i = i + 1u) {
        let cov = clamp(0.5 - min_ds[i] / aa, 0.0, 1.0);
        let a = cov * band_a;
        acc = over_pm(vec4<f32>(colors[i] * a, a), acc);
    }
    return acc;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = in.pt;
    let aa = max(fwidth(p.x), 0.001);
    let kind = u32(in.shape_meta.x + 0.5);
    let stroke_w = in.shape_meta.y;
    let radius = in.shape_meta.z;
    let extra = in.shape_meta.w;

    if kind == 0u {
        return premul(in.fill);
    }
    if kind == 1u {
        let half_ext = in.p0.zw * 0.5;
        let d = sd_rounded_box(p - (in.p0.xy + half_ext), half_ext, radius);
        let f_cov = clamp(0.5 - d / aa, 0.0, 1.0);
        var res = premul(in.fill) * f_cov;
        if stroke_w > 0.0 && in.stroke.a > 0.001 {
            let s_cov = clamp(0.5 - (abs(d) - stroke_w * 0.5) / aa, 0.0, 1.0);
            res = over_pm(premul(in.stroke) * s_cov, res);
        }
        return res;
    }
    if kind == 2u {
        let half_ext = in.p0.zw * 0.5;
        let ctr = in.p0.xy + half_ext;
        let d = sd_rounded_box(p - ctr, half_ext, radius);
        let f_cov = clamp(0.5 - d / aa, 0.0, 1.0);
        let off = vec2<f32>(2.12132, 2.12132);
        let a1 = clamp(0.5 - sd_rounded_box(p - off - ctr, half_ext, radius) / 3.6, 0.0, 1.0);
        let a2 = clamp(0.5 - sd_rounded_box(p + off - ctr, half_ext, radius) / 3.6, 0.0, 1.0);
        let diff = a2 - a1;
        var rgb = in.fill.rgb;
        if diff > 0.0 {
            rgb = mix(rgb, vec3<f32>(1.0), clamp(diff * 0.5, 0.0, 1.0));
        } else if diff < 0.0 {
            rgb = mix(rgb, vec3<f32>(0.188235, 0.188235, 0.286275), clamp(-diff * 0.5, 0.0, 1.0));
        }
        let body = vec4<f32>(rgb * in.fill.a, in.fill.a) * f_cov;
        let s_cov = clamp(0.5 - (abs(d) - 2.0) / aa, 0.0, 1.0);
        return over_pm(premul(in.stroke) * s_cov, body);
    }
    if kind == 3u {
        let dist = length(p - in.p0.xy);
        if extra > 0.5 {
            let t = clamp(dist / max(radius, 1e-4), 0.0, 1.0);
            let s1 = 35.0 / 255.0;
            let s2 = 95.0 / 255.0;
            let s3 = 145.0 / 255.0;
            let s4 = 250.0 / 255.0;
            let a0 = select(0.60, 1.0, extra > 1.5);
            let a1 = select(0.44, 0.54, extra > 1.5);
            var g = 1.0;
            var a = 0.0;
            if t < s1 {
                let u = t / s1;
                g = mix(1.0, 0.8, u);
                a = mix(a0, a1, u);
            } else if t < s2 {
                let u = (t - s1) / (s2 - s1);
                g = mix(0.8, 0.733333, u);
                a = mix(a1, 0.23, u);
            } else if t < s3 {
                let u = (t - s2) / (s3 - s2);
                g = 0.733333;
                a = mix(0.23, 0.10, u);
            } else if t < s4 {
                let u = (t - s3) / (s4 - s3);
                g = mix(0.733333, 1.0, u);
                a = mix(0.10, 0.0, u);
            }
            let edge = clamp(0.5 - (dist - radius) / aa, 0.0, 1.0);
            let fa = a * edge * in.fill.a;
            return vec4<f32>(g * fa, g * fa, g * fa, fa);
        }
        let d = dist - radius;
        if stroke_w > 0.0 {
            return premul(in.stroke) * clamp(0.5 - (abs(d) - stroke_w * 0.5) / aa, 0.0, 1.0);
        }
        return premul(in.fill) * clamp(0.5 - d / aa, 0.0, 1.0);
    }
    if kind == 4u {
        let d = sd_segment(p, in.p0.xy, in.p0.zw) - radius;
        return premul(in.fill) * clamp(0.5 - d / aa, 0.0, 1.0);
    }
    if kind == 5u {
        let half_ext = in.p0.zw * 0.5;
        let ctr = in.p0.xy + half_ext;
        let bot = in.p0.y + in.p0.w;
        let tri_h = max(in.p1.z, 1e-4);
        let inset = min(tri_h, in.p0.w * 0.45);
        let base_hw = in.p1.y * (tri_h + inset) / tri_h;
        let v0 = vec2<f32>(in.p1.x - base_hw, bot - inset);
        let v1 = vec2<f32>(in.p1.x + base_hw, bot - inset);
        let v2 = vec2<f32>(in.p1.x, bot + tri_h);
        let d = sd_tut_bubble(p, ctr, half_ext, radius, v0, v1, v2);
        let f_cov = clamp(0.5 - d / aa, 0.0, 1.0);
        let sc = max(extra, 0.01);
        let off = vec2<f32>(2.828427 * sc, 2.828427 * sc);
        let blur = max(3.6 * sc, 0.5);
        let a1 = clamp(0.5 - sd_tut_bubble(p - off, ctr, half_ext, radius, v0, v1, v2) / blur, 0.0, 1.0);
        let a2 = clamp(0.5 - sd_tut_bubble(p + off, ctr, half_ext, radius, v0, v1, v2) / blur, 0.0, 1.0);
        let diff = a2 - a1;
        var rgb = in.fill.rgb;
        if diff > 0.0 {
            rgb = mix(rgb, vec3<f32>(1.0), clamp(diff, 0.0, 1.0));
        } else if diff < 0.0 {
            rgb = mix(rgb, vec3<f32>(0.188235, 0.188235, 0.286275), clamp(-diff * 0.9, 0.0, 1.0));
        }
        let body = vec4<f32>(rgb * in.fill.a, in.fill.a) * f_cov;
        let s_cov = clamp(0.5 - (abs(d) - stroke_w * 0.5) / aa, 0.0, 1.0);
        return over_pm(premul(in.stroke) * s_cov, body);
    }
    return eval_rainbow(p, extra, in.fill.a, aa);
}
