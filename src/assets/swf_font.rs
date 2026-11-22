// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::swf_geom::BitReader;
use std::collections::HashMap;

/// Vector outline operation in SWF DefineFont3 20480-units-per-EM coordinate space.
#[derive(Debug, Clone, Copy)]
pub enum GlyphOp {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
}

#[derive(Debug, Clone)]
pub struct SwfGlyph {
    pub advance: f32,
    pub ops: Vec<GlyphOp>,
}

/// Embedded SWF DefineFont3 font face (`NEUROPOL`, `Pricedown`, `Helvetica`, etc.).
#[derive(Debug, Clone)]
pub struct SwfFont {
    pub name: String,
    pub ascent: f32,
    pub descent: f32,
    pub glyphs: HashMap<char, SwfGlyph>,
}

impl SwfFont {
    /// Emits EM unit scaling factor to convert DefineFont3 twips to pixel coordinates.
    pub fn scale_for_size(px: f32) -> f32 {
        px / 20480.0
    }
}

pub fn parse_define_font3(td: &[u8]) -> Option<SwfFont> {
    if td.len() < 7 {
        return None;
    }
    let flags = td[2];
    let has_layout = (flags & 0x80) != 0;
    let wide_offsets = (flags & 0x08) != 0;
    let wide_codes = (flags & 0x04) != 0;
    let nlen = td[4] as usize;
    if 5 + nlen + 2 > td.len() {
        return None;
    }
    let raw_name = &td[5..5 + nlen];
    let clean_name = String::from_utf8_lossy(raw_name)
        .trim_matches('\0')
        .trim()
        .to_string();

    let p = 5 + nlen;
    let ng = u16::from_le_bytes([td[p], td[p + 1]]) as usize;
    if ng == 0 {
        return None;
    }
    let off_base = p + 2;
    let off_step = if wide_offsets { 4 } else { 2 };
    if off_base + (ng + 1) * off_step > td.len() {
        return None;
    }

    let read_off = |i: usize| -> usize {
        let q = off_base + i * off_step;
        if wide_offsets {
            u32::from_le_bytes([td[q], td[q + 1], td[q + 2], td[q + 3]]) as usize
        } else {
            u16::from_le_bytes([td[q], td[q + 1]]) as usize
        }
    };

    let code_start = off_base + read_off(ng);
    let code_step = if wide_codes { 2 } else { 1 };
    let advances_start = code_start + ng * code_step;
    let (ascent, descent) = if has_layout && advances_start + 6 <= td.len() {
        let asc = i16::from_le_bytes([td[advances_start], td[advances_start + 1]]) as f32;
        let desc = i16::from_le_bytes([td[advances_start + 2], td[advances_start + 3]]) as f32;
        (asc, desc)
    } else {
        (16384.0, 4096.0)
    };

    let mut glyphs = HashMap::with_capacity(ng);
    for i in 0..ng {
        let cp_pos = code_start + i * code_step;
        if cp_pos + code_step > td.len() {
            break;
        }
        let code = if wide_codes {
            u16::from_le_bytes([td[cp_pos], td[cp_pos + 1]]) as u32
        } else {
            td[cp_pos] as u32
        };
        let Some(ch) = char::from_u32(code) else {
            continue;
        };

        let g_start = off_base + read_off(i);
        let g_end = off_base + read_off(i + 1);
        let ops = if g_start < g_end && g_end <= code_start && g_end <= td.len() {
            parse_glyph_shape(&td[g_start..g_end])
        } else {
            Vec::new()
        };

        let adv = if has_layout && advances_start + 6 + (i + 1) * 2 <= td.len() {
            let ap = advances_start + 6 + i * 2;
            i16::from_le_bytes([td[ap], td[ap + 1]]) as f32
        } else {
            10240.0
        };

        glyphs.insert(ch, SwfGlyph { advance: adv, ops });
    }

    Some(SwfFont {
        name: clean_name,
        ascent,
        descent,
        glyphs,
    })
}

fn parse_glyph_shape(gb: &[u8]) -> Vec<GlyphOp> {
    if gb.is_empty() {
        return Vec::new();
    }
    let mut br = BitReader::new(gb, 0);
    let fb = br.read_ub(4) as u8;
    let lb = br.read_ub(4) as u8;
    let mut x = 0i32;
    let mut y = 0i32;
    let mut ops = Vec::new();

    loop {
        let type_flag = br.read_ub(1);
        if type_flag == 0 {
            let states = br.read_ub(5);
            if states == 0 {
                break;
            }
            if (states & 0x01) != 0 {
                let mb = br.read_ub(5) as u8;
                x = br.read_sb(mb);
                y = br.read_sb(mb);
                ops.push(GlyphOp::Move(x as f32, y as f32));
            }
            if (states & 0x02) != 0 {
                let _ = br.read_ub(fb);
            }
            if (states & 0x04) != 0 {
                let _ = br.read_ub(fb);
            }
            if (states & 0x08) != 0 {
                let _ = br.read_ub(lb);
            }
        } else {
            let straight = br.read_ub(1) != 0;
            let nb = (br.read_ub(4) as u8) + 2;
            if straight {
                let general = br.read_ub(1) != 0;
                let (dx, dy) = if general {
                    (br.read_sb(nb), br.read_sb(nb))
                } else if br.read_ub(1) != 0 {
                    (0, br.read_sb(nb))
                } else {
                    (br.read_sb(nb), 0)
                };
                x += dx;
                y += dy;
                ops.push(GlyphOp::Line(x as f32, y as f32));
            } else {
                let cx = x + br.read_sb(nb);
                let cy = y + br.read_sb(nb);
                x = cx + br.read_sb(nb);
                y = cy + br.read_sb(nb);
                ops.push(GlyphOp::Quad(cx as f32, cy as f32, x as f32, y as f32));
            }
        }
    }
    ops
}
