// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::{AssetBundle, RgbaImage};
use std::fs;
use std::io::Cursor;
use std::path::{Component, Path};

fn validate_out_dir(out_dir: &Path) -> Result<(), String> {
    if out_dir.as_os_str().is_empty() || out_dir.parent().is_none() || out_dir.file_name().is_none()
    {
        return Err(format!(
            "Refusing unsafe output directory '{}'",
            out_dir.display()
        ));
    }
    Ok(())
}

fn validate_asset_name(name: &str) -> Result<&str, String> {
    if name.is_empty() || name.contains(['/', '\\', '\0']) {
        return Err(format!("Unsafe asset filename '{}'", name));
    }
    let mut comps = Path::new(name).components();
    match (comps.next(), comps.next()) {
        (Some(Component::Normal(_)), None) => Ok(name),
        _ => Err(format!("Unsafe asset path component '{}'", name)),
    }
}

pub fn extract_assets_to_dir(bundle: &AssetBundle, out_dir: &Path) -> Result<(), String> {
    validate_out_dir(out_dir)?;
    let img_dir = out_dir.join("images");
    let snd_dir = out_dir.join("sounds");
    let lvl_dir = out_dir.join("levels");
    fs::create_dir_all(&img_dir).map_err(|e| format!("mkdir error: {}", e))?;
    fs::create_dir_all(&snd_dir).map_err(|e| format!("mkdir error: {}", e))?;
    fs::create_dir_all(&lvl_dir).map_err(|e| format!("mkdir error: {}", e))?;

    for (name, img) in &bundle.images {
        let safe = validate_asset_name(name)?;
        let path = img_dir.join(format!("{}.png", safe));
        let Some(ibuf) = image::RgbaImage::from_raw(img.width, img.height, img.data.clone()) else {
            return Err(format!("invalid RGBA buffer size for '{}'", name));
        };
        ibuf.save_with_format(&path, image::ImageFormat::Png)
            .map_err(|e| format!("png write {:?}: {}", path, e))?;
    }
    for (name, mp3) in &bundle.sounds {
        let safe = validate_asset_name(name)?;
        let path = snd_dir.join(format!("{}.mp3", safe));
        fs::write(&path, mp3).map_err(|e| format!("write {:?}: {}", path, e))?;
    }
    for (name, xml) in &bundle.raw_xmls {
        let safe = validate_asset_name(name)?;
        let path = lvl_dir.join(format!("{}.xml", safe));
        fs::write(&path, xml).map_err(|e| format!("write {:?}: {}", path, e))?;
    }
    Ok(())
}

pub fn decode_jpeg_rgba(mut bytes: &[u8]) -> Result<RgbaImage, String> {
    if bytes.starts_with(&[0xFF, 0xD9, 0xFF, 0xD8]) {
        bytes = &bytes[4..];
    }
    let cleaned_buf: Vec<u8>;
    let slice: &[u8] =
        if let Some(pos) = bytes.windows(4).position(|w| w == [0xFF, 0xD9, 0xFF, 0xD8]) {
            let mut merged = Vec::with_capacity(bytes.len().saturating_sub(4));
            merged.extend_from_slice(&bytes[..pos]);
            merged.extend_from_slice(&bytes[pos + 4..]);
            cleaned_buf = merged;
            &cleaned_buf
        } else {
            bytes
        };
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(slice));
    let pixels = decoder
        .decode()
        .map_err(|e| format!("JPEG decode error: {}", e))?;
    let info = decoder.info().ok_or("Missing JPEG info")?;
    let w = info.width as u32;
    let h = info.height as u32;
    let npix = (w as usize) * (h as usize);
    let mut rgba = vec![255u8; npix * 4];
    match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            for i in 0..npix {
                rgba[i * 4] = pixels[i * 3];
                rgba[i * 4 + 1] = pixels[i * 3 + 1];
                rgba[i * 4 + 2] = pixels[i * 3 + 2];
            }
        }
        jpeg_decoder::PixelFormat::L8 => {
            for i in 0..npix {
                let v = pixels[i];
                rgba[i * 4] = v;
                rgba[i * 4 + 1] = v;
                rgba[i * 4 + 2] = v;
            }
        }
        jpeg_decoder::PixelFormat::CMYK32 => {
            for i in 0..npix {
                let c = pixels[i * 4] as f32 / 255.0;
                let m = pixels[i * 4 + 1] as f32 / 255.0;
                let y = pixels[i * 4 + 2] as f32 / 255.0;
                let k = pixels[i * 4 + 3] as f32 / 255.0;
                rgba[i * 4] = ((1.0 - c) * (1.0 - k) * 255.0) as u8;
                rgba[i * 4 + 1] = ((1.0 - m) * (1.0 - k) * 255.0) as u8;
                rgba[i * 4 + 2] = ((1.0 - y) * (1.0 - k) * 255.0) as u8;
            }
        }
        other => return Err(format!("Unsupported JPEG pixel format {:?}", other)),
    }
    Ok(RgbaImage {
        width: w,
        height: h,
        data: rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::{validate_asset_name, validate_out_dir};
    use std::path::Path;

    #[test]
    fn guards_block_unsafe_paths() {
        assert!(validate_out_dir(Path::new("")).is_err());
        assert!(validate_out_dir(Path::new("/")).is_err());
        assert!(validate_out_dir(Path::new(".")).is_err());
        assert!(validate_out_dir(Path::new("..")).is_err());
        assert!(validate_out_dir(Path::new("extracted_assets")).is_ok());
        assert!(validate_out_dir(Path::new("/tmp/aqualux_extracted")).is_ok());

        for bad in [
            "",
            ".",
            "..",
            "../escape",
            "a/b",
            "a\\b",
            "/etc/passwd",
            "bad\0name",
        ] {
            assert!(
                validate_asset_name(bad).is_err(),
                "expected err for {bad:?}"
            );
        }
        assert_eq!(
            validate_asset_name("PipeSpigotBitmap").unwrap(),
            "PipeSpigotBitmap"
        );
    }
}
