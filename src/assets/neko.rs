// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

/// Extracts the embedded SWF asset payload from NekoVM `resource.n` bytecode.
pub fn extract_swf_from_neko(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 16 || &data[..4] != b"NEKO" {
        return Err("Invalid NEKO magic header".to_string());
    }
    let nglobals = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let mut pos = 16usize;
    let mut chunks: Vec<&[u8]> = Vec::new();
    let mut collecting = false;

    for _ in 0..nglobals {
        if pos >= data.len() {
            return Err("Unexpected EOF parsing NEKO globals".to_string());
        }
        let tag = data[pos];
        pos += 1;
        match tag {
            1 => {
                // GLOBAL_VAR: null-terminated
                let end = data[pos..]
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or("Unterminated NEKO VAR global")?;
                pos += end + 1;
            }
            2 => {
                // GLOBAL_FUNCTION: 4 bytes
                pos += 4;
            }
            3 => {
                // GLOBAL_STRING: u16 LE len + bytes
                if pos + 2 > data.len() {
                    return Err("Truncated NEKO string length".to_string());
                }
                let len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
                pos += 2;
                if pos + len > data.len() {
                    return Err("Truncated NEKO string payload".to_string());
                }
                let slice = &data[pos..pos + len];
                pos += len;

                if !collecting {
                    if slice.len() >= 4 && (&slice[..3] == b"CWS" || &slice[..3] == b"FWS") {
                        collecting = true;
                        chunks.push(slice);
                        if len < 65535 {
                            break;
                        }
                    }
                } else {
                    chunks.push(slice);
                    if len < 65535 {
                        break;
                    }
                }
            }
            4 => {
                // GLOBAL_FLOAT: null-terminated
                let end = data[pos..]
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or("Unterminated NEKO FLOAT global")?;
                pos += end + 1;
            }
            5 => {
                // GLOBAL_DEBUG
                pos = skip_neko_debug(data, pos)?;
            }
            6 => {
                // GLOBAL_VERSION
                pos += 1;
            }
            other => return Err(format!("Unknown NEKO global type {}", other)),
        }
    }

    if chunks.is_empty() {
        return Err("No embedded SWF resource found in NEKO globals".to_string());
    }
    let total: usize = chunks.iter().map(|c| c.len()).sum();
    let mut out = Vec::with_capacity(total);
    for c in chunks {
        out.extend_from_slice(c);
    }
    Ok(out)
}

fn skip_neko_debug(data: &[u8], mut pos: usize) -> Result<usize, String> {
    if pos >= data.len() {
        return Err("Truncated NEKO DEBUG header".to_string());
    }
    let mut nfiles = data[pos] as usize;
    pos += 1;
    if nfiles >= 0x80 {
        if pos >= data.len() {
            return Err("Truncated NEKO DEBUG file count".to_string());
        }
        nfiles = ((nfiles & 0x7F) << 8) | (data[pos] as usize);
        pos += 1;
    }
    for _ in 0..nfiles {
        let end = data[pos..]
            .iter()
            .position(|&b| b == 0)
            .ok_or("Unterminated NEKO DEBUG filename")?;
        pos += end + 1;
    }
    if pos + 4 > data.len() {
        return Err("Truncated NEKO DEBUG npos".to_string());
    }
    let npos =
        u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
    pos += 4;
    let mut count = 0usize;
    while count < npos {
        if pos >= data.len() {
            return Err("Truncated NEKO DEBUG entries".to_string());
        }
        let b = data[pos];
        pos += 1;
        if b & 1 != 0 {
            pos += 1;
        } else if b & 2 != 0 {
            count += ((b >> 2) & 15) as usize;
        } else if b & 4 != 0 {
            count += 1;
        } else {
            pos += 2;
            count += 1;
        }
    }
    Ok(pos)
}
