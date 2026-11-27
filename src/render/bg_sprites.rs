// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::{AssetBundle, Matrix2D, SpriteItem};
use crate::render::backgrounds::BgAnimState;
use crate::render::canvas::Canvas;
use crate::render::easing::{bounce_out, quad_out};
use crate::render::tex_bank::TexBank;
use rustc_hash::FxHashMap;

fn layer_colorize_delay(layer: Option<&str>, snowy: bool) -> f32 {
    if snowy {
        match layer {
            Some("backSnowHillsLayer") => 2.0,
            Some("frontSnowHillsLayer") => 2.1,
            Some("snowCloudsLayer") => 2.2,
            _ => 2.0,
        }
    } else {
        match layer {
            Some("backHillsLayer") => 2.8,
            Some("distantCloudsLayer") => 2.9,
            Some("midCloudsLayer") => 3.0,
            Some("frontCloudsLayer") => 3.1,
            Some("frontHillsLayer") => 3.2,
            _ => 2.8,
        }
    }
}

/// Formats an SWF twip-quantized coordinate using AS3's default `String(Number)` formatting
/// so `Array.sortOn("x")` matches Flash's default lexicographic string order.
pub(crate) fn as3_number_string(v: f32) -> String {
    let clean = (v * 20.0).round() / 20.0;
    let norm = if clean == 0.0 { 0.0 } else { clean };
    format!("{norm}")
}

pub(crate) fn draw_popup_sprite<F>(
    canvas: &mut Canvas,
    bank: &TexBank,
    items: &[SpriteItem],
    anim: &BgAnimState,
    snowy: bool,
    layer_alpha: f32,
    mut pred: F,
) where
    F: FnMut(Option<&str>) -> bool,
{
    let alpha = layer_alpha.clamp(0.0, 1.0);
    if alpha <= 0.004 {
        return;
    }
    let selected: Vec<(usize, &SpriteItem)> = items
        .iter()
        .enumerate()
        .filter(|(_, it)| pred(it.layer_name.as_deref()))
        .collect();
    // Deduplicate direct children per layer by `(child_idx, child_x)` before AS3 lexicographic `sortOn("x")`.
    let mut layer_children: FxHashMap<&str, Vec<(u32, f32)>> = FxHashMap::default();
    for (enum_idx, item) in &selected {
        let lname = item.layer_name.as_deref().unwrap_or("");
        let key = item
            .popup
            .map(|p| (p.child_idx, p.child_x))
            .unwrap_or((*enum_idx as u32, item.matrix.tx));
        let vec = layer_children.entry(lname).or_default();
        if !vec.iter().any(|&(idx, _)| idx == key.0) {
            vec.push(key);
        }
    }
    let mut layer_ranks: FxHashMap<(&str, u32), usize> = FxHashMap::default();
    for (lname, mut children) in layer_children {
        children.sort_by_key(|&(idx, _)| idx);
        children.sort_by_key(|a| as3_number_string(a.1));
        for (rank, (child_idx, _)) in children.into_iter().enumerate() {
            layer_ranks.insert((lname, child_idx), rank);
        }
    }
    for (enum_idx, item) in selected {
        let lname = item.layer_name.as_deref().unwrap_or("");
        let (child_idx, child_y, layer_c, layer_d) = item
            .popup
            .map(|p| (p.child_idx, p.child_y, p.layer_c, p.layer_d))
            .unwrap_or((enum_idx as u32, item.matrix.ty, 0.0, 1.0));
        let rank = *layer_ranks.get(&(lname, child_idx)).unwrap_or(&0);

        let base_delay = if snowy && lname == "frontSnowHillsLayer" {
            0.0
        } else {
            1.0
        };
        let dur = if snowy && lname == "frontSnowHillsLayer" {
            3.0
        } else if snowy {
            2.0
        } else {
            1.5
        };
        let y_t = ((anim.age - (base_delay + rank as f32 * 0.15)) / dur).clamp(0.0, 1.0);
        let dy_local = (1.0 - bounce_out(y_t)) * (800.0 - child_y);

        let col_delay = layer_colorize_delay(item.layer_name.as_deref(), snowy);
        let intro_shade = quad_out(((anim.age - col_delay) / 2.0).clamp(0.0, 1.0));
        let shade = intro_shade * (1.0 - anim.darken);

        let Some((tid, w, h)) = bank.cid(item.bitmap_cid) else {
            continue;
        };
        let mut m = item.matrix;
        m.tx += layer_c * dy_local;
        m.ty += layer_d * dy_local;
        canvas.draw_tex_matrix(tid, m, w as f32, h as f32, None, shade, alpha);
    }
}

/// Renders title logo assembly: pipe pieces rise as black silhouettes from `y=600` (`1.5..2.65s`), then reveal and illuminate `trueTitle` over `3.5..5.7s`.
pub fn draw_sprite_title(
    canvas: &mut Canvas,
    assets: &AssetBundle,
    bank: &TexBank,
    cx: f32,
    cy: f32,
    title_age: f32,
) {
    let Some(items) = assets.sprites.get("SpriteTitle") else {
        return;
    };
    let base_m = Matrix2D {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: cx,
        ty: cy,
    };
    let true_revealed = title_age >= 3.5;
    let color_shade = quad_out(((title_age - 3.5) / 2.2).clamp(0.0, 1.0));
    let draw_shadow = |canvas: &mut Canvas, cid: u16, m: Matrix2D| {
        if let Some((sh_id, sw, sh, pad)) = bank.cid_shadow(cid) {
            let mut sh_base = base_m;
            sh_base.tx += 8.0;
            let pad_m = Matrix2D {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                tx: -(pad as f32),
                ty: -(pad as f32),
            };
            canvas.draw_tex_matrix(
                sh_id,
                sh_base.mul(m).mul(pad_m),
                sw as f32,
                sh as f32,
                None,
                1.0,
                1.0,
            );
        }
    };

    for (idx, item) in items.iter().enumerate() {
        let is_true = item.layer_name.as_deref() == Some("trueTitle");
        if is_true {
            if true_revealed && let Some((tid, w, h)) = bank.cid(item.bitmap_cid) {
                draw_shadow(canvas, item.bitmap_cid, item.matrix);
                canvas.draw_tex_matrix(
                    tid,
                    base_m.mul(item.matrix),
                    w as f32,
                    h as f32,
                    None,
                    color_shade,
                    1.0,
                );
                canvas.sheen_last_sprite(1.42);
            }
        } else if !true_revealed {
            let delay = 1.5 + (((idx * 37) % 23) as f32 / 22.0) * 1.15;
            let p = ((title_age - delay) / 1.0).clamp(0.0, 1.0);
            if title_age >= delay
                && let Some((tid, w, h)) = bank.cid(item.bitmap_cid)
            {
                let eased = quad_out(p);
                let mut m = item.matrix;
                m.ty = 600.0 + (item.matrix.ty - 600.0) * eased;
                draw_shadow(canvas, item.bitmap_cid, m);
                canvas.draw_tex_matrix(tid, base_m.mul(m), w as f32, h as f32, None, 0.0, 1.0);
            }
        }
    }
}
