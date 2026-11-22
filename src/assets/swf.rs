// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::swf_decode::{
    decode_jpeg_with_tables, decode_jpeg3_rgba, decode_swf_lossless, decompress_swf_body,
    parse_symbol_table,
};
use crate::assets::swf_font::{SwfFont, parse_define_font3};
use crate::assets::swf_geom::Matrix2D;
use crate::assets::swf_sprite::{SpriteItem, flatten_sprite, parse_shape_bitmap_fills};
use crate::assets::{AssetBundle, RgbaImage};
use crate::game::parse_levels_xml;
use rayon::prelude::*;
use std::collections::HashMap;

struct SwfBitmapJpeg {
    cid: u16,
    jpg_bytes: Vec<u8>,
    alpha_zlib: Option<Vec<u8>>,
    needs_tables: bool,
}

pub fn load_bundle_from_swf(bytes: &[u8], label: &str) -> Result<AssetBundle, String> {
    let body = decompress_swf_body(bytes)?;
    if body.len() < 5 {
        return Err("SWF body truncated".to_string());
    }
    let nbits = (body[0] >> 3) as usize;
    let rect_bytes = (5 + 4 * nbits).div_ceil(8);
    let mut tpos = rect_bytes + 4;

    let mut sym_map: HashMap<u16, Vec<String>> = HashMap::new();
    let mut jpeg_tables: Option<Vec<u8>> = None;
    let mut raw_lossless: Vec<(u16, bool, Vec<u8>)> = Vec::new();
    let mut raw_jpegs: Vec<SwfBitmapJpeg> = Vec::new();
    let mut raw_sounds: Vec<(u16, Vec<u8>)> = Vec::new();
    let mut raw_bins: Vec<(u16, Vec<u8>)> = Vec::new();
    let mut shape_fills: HashMap<u16, Vec<(u16, Matrix2D)>> = HashMap::new();
    let mut raw_sprites: HashMap<u16, Vec<u8>> = HashMap::new();
    let mut fonts: HashMap<String, SwfFont> = HashMap::new();

    while tpos + 2 <= body.len() {
        let th = u16::from_le_bytes([body[tpos], body[tpos + 1]]);
        tpos += 2;
        let tag_type = th >> 6;
        let mut tag_len = (th & 0x3F) as usize;
        if tag_len == 0x3F {
            if tpos + 4 > body.len() {
                break;
            }
            tag_len =
                u32::from_le_bytes([body[tpos], body[tpos + 1], body[tpos + 2], body[tpos + 3]])
                    as usize;
            tpos += 4;
        }
        if tpos + tag_len > body.len() {
            break;
        }
        let td = &body[tpos..tpos + tag_len];
        tpos += tag_len;

        match tag_type {
            0 => break,
            2 if td.len() >= 4 => {
                let cid = u16::from_le_bytes([td[0], td[1]]);
                shape_fills.insert(cid, parse_shape_bitmap_fills(td));
            }
            6 if td.len() > 2 => {
                let cid = u16::from_le_bytes([td[0], td[1]]);
                raw_jpegs.push(SwfBitmapJpeg {
                    cid,
                    jpg_bytes: td[2..].to_vec(),
                    alpha_zlib: None,
                    needs_tables: true,
                });
            }
            8 if !td.is_empty() => jpeg_tables = Some(td.to_vec()),
            14 if td.len() > 9 => {
                let cid = u16::from_le_bytes([td[0], td[1]]);
                if ((td[2] >> 4) & 0x0F) == 2 {
                    raw_sounds.push((cid, td[9..].to_vec()));
                }
            }
            20 | 36 if td.len() >= 7 => {
                let cid = u16::from_le_bytes([td[0], td[1]]);
                raw_lossless.push((cid, tag_type == 36, td.to_vec()));
            }
            21 if td.len() > 2 => {
                let cid = u16::from_le_bytes([td[0], td[1]]);
                raw_jpegs.push(SwfBitmapJpeg {
                    cid,
                    jpg_bytes: td[2..].to_vec(),
                    alpha_zlib: None,
                    needs_tables: false,
                });
            }
            35 if td.len() > 6 => {
                let cid = u16::from_le_bytes([td[0], td[1]]);
                let alpha_off = u32::from_le_bytes([td[2], td[3], td[4], td[5]]) as usize;
                let end = (6 + alpha_off).min(td.len());
                raw_jpegs.push(SwfBitmapJpeg {
                    cid,
                    jpg_bytes: td[6..end].to_vec(),
                    alpha_zlib: Some(td[end..].to_vec()),
                    needs_tables: false,
                });
            }
            39 if td.len() >= 4 => {
                let cid = u16::from_le_bytes([td[0], td[1]]);
                raw_sprites.insert(cid, td.to_vec());
            }
            56 | 76 if td.len() >= 2 => parse_symbol_table(td, &mut sym_map),
            75 => {
                if let Some(font) = parse_define_font3(td) {
                    fonts.insert(font.name.to_ascii_lowercase(), font);
                }
            }
            87 if td.len() > 6 => {
                let cid = u16::from_le_bytes([td[0], td[1]]);
                raw_bins.push((cid, td[6..].to_vec()));
            }
            _ => {}
        }
    }

    let decoded_lossless: Vec<(u16, RgbaImage)> = raw_lossless
        .into_par_iter()
        .filter_map(|(cid, has_alpha, td)| {
            decode_swf_lossless(&td, has_alpha)
                .ok()
                .map(|img| (cid, img))
        })
        .collect();
    let jt_ref = jpeg_tables.as_deref();
    let decoded_jpegs: Vec<(u16, RgbaImage)> = raw_jpegs
        .into_par_iter()
        .filter_map(|entry| {
            let decoded = if entry.needs_tables {
                decode_jpeg_with_tables(&entry.jpg_bytes, jt_ref)
            } else {
                decode_jpeg3_rgba(&entry.jpg_bytes, entry.alpha_zlib.as_deref())
            };
            decoded.ok().map(|img| (entry.cid, img))
        })
        .collect();

    let mut cid_images: HashMap<u16, RgbaImage> = HashMap::new();
    let mut images: HashMap<String, RgbaImage> = HashMap::new();
    for (cid, img) in decoded_lossless.into_iter().chain(decoded_jpegs) {
        if let Some(names) = sym_map.get(&cid) {
            for name in names {
                images.insert(name.clone(), img.clone());
            }
        }
        cid_images.insert(cid, img);
    }

    let mut sprites: HashMap<String, Vec<SpriteItem>> = HashMap::new();
    for &cid in raw_sprites.keys() {
        if let Some(names) = sym_map.get(&cid) {
            let items = flatten_sprite(cid, &raw_sprites, &shape_fills);
            if !items.is_empty() {
                for name in names {
                    sprites.insert(name.clone(), items.clone());
                }
            }
        }
    }

    let mut sounds = HashMap::new();
    for (cid, mp3) in raw_sounds {
        if let Some(names) = sym_map.get(&cid) {
            for name in names {
                sounds.insert(name.clone(), mp3.clone());
            }
        }
    }

    let mut raw_xmls = HashMap::new();
    let mut campaign_levels = Vec::new();
    let mut lotd_levels = Vec::new();
    for (cid, bin) in raw_bins {
        let names = sym_map
            .get(&cid)
            .cloned()
            .unwrap_or_else(|| vec![format!("bin_{}", cid)]);
        if let Ok(text) = std::str::from_utf8(&bin) {
            for name in &names {
                raw_xmls.insert(name.clone(), text.to_string());
            }
            if let Ok(parsed) = parse_levels_xml(text) {
                if names
                    .iter()
                    .any(|n| n.to_ascii_lowercase().contains("lotd"))
                {
                    lotd_levels.extend(parsed);
                } else if !parsed.is_empty() {
                    campaign_levels.extend(parsed);
                }
            }
        }
    }

    let bundle = AssetBundle {
        source_label: label.to_string(),
        images,
        cid_images,
        sprites,
        fonts,
        sounds,
        raw_xmls,
        campaign_levels,
        lotd_levels,
    };
    bundle.validate()?;
    Ok(bundle)
}
