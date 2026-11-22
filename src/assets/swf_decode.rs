// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::dir_loader::decode_jpeg_rgba;
use crate::assets::{RgbaImage, short_asset_name};
use flate2::read::ZlibDecoder;
use std::collections::HashMap;
use std::io::Read;

pub fn decompress_swf_body(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() < 8 {
        return Err("SWF too short".to_string());
    }
    if bytes.starts_with(b"CWS") {
        let mut dec = ZlibDecoder::new(&bytes[8..]);
        let mut out = Vec::new();
        dec.read_to_end(&mut out)
            .map_err(|e| format!("SWF zlib decompress failed: {}", e))?;
        Ok(out)
    } else if bytes.starts_with(b"FWS") {
        Ok(bytes[8..].to_vec())
    } else {
        Err("Invalid SWF signature".to_string())
    }
}

pub fn parse_symbol_table(td: &[u8], sym_map: &mut HashMap<u16, Vec<String>>) {
    let count = u16::from_le_bytes([td[0], td[1]]) as usize;
    let mut p = 2;
    for _ in 0..count {
        if p + 2 > td.len() {
            break;
        }
        let cid = u16::from_le_bytes([td[p], td[p + 1]]);
        p += 2;
        if let Some(rel_end) = td[p..].iter().position(|&b| b == 0) {
            let full_name = String::from_utf8_lossy(&td[p..p + rel_end]).to_string();
            let short = short_asset_name(&full_name);
            let list = sym_map.entry(cid).or_default();
            if !list.contains(&short) {
                list.push(short);
            }
            p += rel_end + 1;
        } else {
            break;
        }
    }
}

pub fn decode_jpeg_with_tables(jpg: &[u8], tables: Option<&[u8]>) -> Result<RgbaImage, String> {
    let Some(t) = tables else {
        return decode_jpeg_rgba(jpg);
    };
    if t.len() >= 4
        && jpg.len() >= 4
        && t.ends_with(&[0xFF, 0xD9])
        && jpg.starts_with(&[0xFF, 0xD8])
    {
        let mut merged = Vec::with_capacity(t.len() - 2 + jpg.len() - 2);
        merged.extend_from_slice(&t[..t.len() - 2]);
        merged.extend_from_slice(&jpg[2..]);
        decode_jpeg_rgba(&merged)
    } else {
        decode_jpeg_rgba(jpg)
    }
}

pub fn decode_jpeg3_rgba(jpg: &[u8], alpha_zlib: Option<&[u8]>) -> Result<RgbaImage, String> {
    let mut img = decode_jpeg_rgba(jpg)?;
    let Some(az) = alpha_zlib else {
        return Ok(img);
    };
    if az.is_empty() {
        return Ok(img);
    }
    let mut dec = ZlibDecoder::new(az);
    let mut alpha = Vec::new();
    dec.read_to_end(&mut alpha)
        .map_err(|e| format!("JPEG3 alpha zlib error: {}", e))?;
    let npix = (img.width as usize) * (img.height as usize);
    if alpha.len() < npix {
        return Ok(img);
    }

    let mut max_excess = 0i32;
    let mut sample_semi = 0usize;
    for (i, &a) in alpha.iter().take(npix).enumerate() {
        if a > 4 && a < 180 {
            sample_semi += 1;
            let s = i * 4;
            let m = img.data[s].max(img.data[s + 1]).max(img.data[s + 2]) as i32;
            max_excess = max_excess.max(m - (a as i32));
        }
    }
    let premultiplied = sample_semi > 4 && max_excess <= 8;
    for (i, &a) in alpha.iter().take(npix).enumerate() {
        let s = i * 4;
        if premultiplied {
            if a == 0 {
                img.data[s..s + 3].fill(0);
            } else if a < 255 {
                let af = a as u32;
                img.data[s] = (((img.data[s] as u32) * 255) / af).min(255) as u8;
                img.data[s + 1] = (((img.data[s + 1] as u32) * 255) / af).min(255) as u8;
                img.data[s + 2] = (((img.data[s + 2] as u32) * 255) / af).min(255) as u8;
            }
        }
        img.data[s + 3] = a;
    }
    Ok(img)
}

pub fn decode_swf_lossless(td: &[u8], has_alpha: bool) -> Result<RgbaImage, String> {
    if td.len() < 7 {
        return Err("Lossless tag too short".to_string());
    }
    let format = td[2];
    let width = u16::from_le_bytes([td[3], td[4]]) as u32;
    let height = u16::from_le_bytes([td[5], td[6]]) as u32;
    if format != 5 {
        return Err(format!("Unsupported SWF lossless format {}", format));
    }
    let mut dec = ZlibDecoder::new(&td[7..]);
    let mut raw = Vec::new();
    dec.read_to_end(&mut raw)
        .map_err(|e| format!("Lossless zlib error: {}", e))?;

    let expected = (width as usize) * (height as usize) * 4;
    if raw.len() < expected {
        return Err("Decompressed lossless bitmap too short".to_string());
    }

    let mut rgba = vec![0u8; expected];
    for i in 0..(width as usize * height as usize) {
        let s = i * 4;
        if has_alpha {
            let a = raw[s];
            let (r, g, b) = (raw[s + 1], raw[s + 2], raw[s + 3]);
            if a == 0 {
                rgba[s..s + 4].fill(0);
            } else if a == 255 {
                rgba[s..s + 4].copy_from_slice(&[r, g, b, 255]);
            } else {
                let af = a as u32;
                rgba[s] = (((r as u32) * 255) / af).min(255) as u8;
                rgba[s + 1] = (((g as u32) * 255) / af).min(255) as u8;
                rgba[s + 2] = (((b as u32) * 255) / af).min(255) as u8;
                rgba[s + 3] = a;
            }
        } else {
            rgba[s..s + 4].copy_from_slice(&[raw[s + 1], raw[s + 2], raw[s + 3], 255]);
        }
    }

    Ok(RgbaImage {
        width,
        height,
        data: rgba,
    })
}
