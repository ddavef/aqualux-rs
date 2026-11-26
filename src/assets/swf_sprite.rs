// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::swf_geom::{BitReader, Matrix2D};
use std::collections::HashMap;

/// Layer-local metadata of the direct child inside a named layer sprite for `popupLayer`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopupMeta {
    pub child_idx: u32,
    pub child_x: f32,
    pub child_y: f32,
    pub layer_c: f32,
    pub layer_d: f32,
}

/// Flattened leaf bitmap draw instruction extracted from an SWF `DefineSprite`.
#[derive(Debug, Clone)]
pub struct SpriteItem {
    pub bitmap_cid: u16,
    pub layer_name: Option<String>,
    pub depth: u32,
    pub matrix: Matrix2D,
    pub popup: Option<PopupMeta>,
}

pub fn parse_shape_bitmap_fills(td: &[u8]) -> Vec<(u16, Matrix2D)> {
    if td.len() < 4 {
        return Vec::new();
    }
    let mut br = BitReader::new(td, 2);
    br.skip_rect();
    let mut p = br.byte_pos;
    if p >= td.len() {
        return Vec::new();
    }
    let mut fc = td[p] as usize;
    p += 1;
    if fc == 255 {
        if p + 2 > td.len() {
            return Vec::new();
        }
        fc = u16::from_le_bytes([td[p], td[p + 1]]) as usize;
        p += 2;
    }

    let mut fills = Vec::new();
    for _ in 0..fc {
        if p >= td.len() {
            break;
        }
        let ft = td[p];
        p += 1;
        match ft {
            0x00 => p += 3,
            0x40..=0x43 => {
                if p + 2 > td.len() {
                    break;
                }
                let bmp_cid = u16::from_le_bytes([td[p], td[p + 1]]);
                p += 2;
                let mut mbr = BitReader::new(td, p);
                let mut m = mbr.read_matrix();
                p = mbr.byte_pos;
                m.a /= 20.0;
                m.b /= 20.0;
                m.c /= 20.0;
                m.d /= 20.0;
                if bmp_cid != 0xFFFF {
                    fills.push((bmp_cid, m));
                }
            }
            _ => break,
        }
    }
    fills
}

/// Flattens frame 1 of a `DefineSprite` into depth-ordered bitmap instances.
pub fn flatten_sprite(
    root_cid: u16,
    sprites: &HashMap<u16, Vec<u8>>,
    shape_fills: &HashMap<u16, Vec<(u16, Matrix2D)>>,
) -> Vec<SpriteItem> {
    let mut out = Vec::new();
    walk_sprite(
        root_cid,
        Matrix2D::identity(),
        None,
        0,
        0,
        None,
        sprites,
        shape_fills,
        &mut out,
    );
    out.sort_by_key(|item| item.depth);
    out
}

#[allow(clippy::too_many_arguments)]
fn walk_sprite(
    sprite_cid: u16,
    parent_mat: Matrix2D,
    top_layer: Option<&str>,
    depth_level: u8,
    depth_prefix: u32,
    popup_meta: Option<PopupMeta>,
    sprites: &HashMap<u16, Vec<u8>>,
    shape_fills: &HashMap<u16, Vec<(u16, Matrix2D)>>,
    out: &mut Vec<SpriteItem>,
) {
    let Some(td) = sprites.get(&sprite_cid) else {
        return;
    };
    if td.len() < 4 {
        return;
    }
    let mut p = 4usize;
    let mut placed: Vec<(u16, Option<u16>, Matrix2D, Option<String>)> = Vec::new();

    while p + 2 <= td.len() {
        let th = u16::from_le_bytes([td[p], td[p + 1]]);
        p += 2;
        let tag_type = th >> 6;
        let mut tag_len = (th & 0x3F) as usize;
        if tag_len == 0x3F {
            if p + 4 > td.len() {
                break;
            }
            tag_len = u32::from_le_bytes([td[p], td[p + 1], td[p + 2], td[p + 3]]) as usize;
            p += 4;
        }
        if p + tag_len > td.len() {
            break;
        }
        let sub = &td[p..p + tag_len];
        p += tag_len;

        match tag_type {
            0 | 1 => break,
            26 | 70 if !sub.is_empty() => {
                if let Some(entry) = parse_place_object(tag_type, sub) {
                    placed.push(entry);
                }
            }
            _ => {}
        }
    }

    placed.sort_by_key(|e| e.0);
    for (idx, (depth, char_opt, local_mat, name_opt)) in placed.into_iter().enumerate() {
        let Some(cid) = char_opt else {
            continue;
        };
        let combined = parent_mat.mul(local_mat);
        let effective_layer = top_layer.map(|s| s.to_string()).or(name_opt);
        let order_key = if top_layer.is_none() {
            (depth as u32) << 16
        } else {
            depth_prefix | ((idx as u32 + 1) & 0xFFFF)
        };
        let child_popup = if depth_level == 1 && top_layer.is_some() {
            Some(PopupMeta {
                child_idx: idx as u32,
                child_x: local_mat.tx,
                child_y: local_mat.ty,
                layer_c: parent_mat.c,
                layer_d: parent_mat.d,
            })
        } else {
            popup_meta
        };

        if sprites.contains_key(&cid) {
            walk_sprite(
                cid,
                combined,
                effective_layer.as_deref(),
                depth_level.saturating_add(1),
                order_key,
                child_popup,
                sprites,
                shape_fills,
                out,
            );
        } else if let Some(fills) = shape_fills.get(&cid) {
            for &(bmp_cid, fill_mat) in fills {
                out.push(SpriteItem {
                    bitmap_cid: bmp_cid,
                    layer_name: effective_layer.clone(),
                    depth: order_key,
                    matrix: combined.mul(fill_mat),
                    popup: child_popup,
                });
            }
        }
    }
}

fn parse_place_object(
    tag_type: u16,
    sub: &[u8],
) -> Option<(u16, Option<u16>, Matrix2D, Option<String>)> {
    let flags = sub[0];
    let mut sp = 1usize;
    let flags2 = if tag_type == 70 {
        if sp >= sub.len() {
            return None;
        }
        let f = sub[sp];
        sp += 1;
        f
    } else {
        0
    };
    if sp + 2 > sub.len() {
        return None;
    }
    let depth = u16::from_le_bytes([sub[sp], sub[sp + 1]]);
    sp += 2;
    if tag_type == 70
        && (flags2 & 0x08) != 0
        && let Some(end) = sub[sp..].iter().position(|&b| b == 0)
    {
        sp += end + 1;
    }
    let char_id = if (flags & 0x02) != 0 {
        if sp + 2 > sub.len() {
            return None;
        }
        let c = u16::from_le_bytes([sub[sp], sub[sp + 1]]);
        sp += 2;
        Some(c)
    } else {
        None
    };
    let mat = if (flags & 0x04) != 0 {
        let mut br = BitReader::new(sub, sp);
        let m = br.read_matrix();
        sp = br.byte_pos;
        m
    } else {
        Matrix2D::identity()
    };
    if (flags & 0x08) != 0 {
        let mut br = BitReader::new(sub, sp);
        br.skip_cxform_alpha();
        sp = br.byte_pos;
    }
    if (flags & 0x10) != 0 {
        sp = sp.saturating_add(2);
    }
    let name = if (flags & 0x20) != 0 && sp < sub.len() {
        sub[sp..]
            .iter()
            .position(|&b| b == 0)
            .map(|rel| String::from_utf8_lossy(&sub[sp..sp + rel]).to_string())
    } else {
        None
    };
    Some((depth, char_id, mat, name))
}
