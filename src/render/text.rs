// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::{AssetBundle, SwfFont};
use crate::render::canvas::Canvas;
use crate::render::gpu_bake::GpuTexture;
use crate::render::gpu_core::GpuCore;
use crate::render::gpu_text::bake_swf_text_gpu;
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::sync::Arc;
use wgpu::BindGroup;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontFamily {
    Neuropol,
    Pricedown,
    Helvetica,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TextMetaKey {
    fam: FontFamily,
    size_q: u32,
    sx_q: u32,
    sy_q: u32,
    dpr_q: u32,
    color: (u8, u8, u8),
    stroke: Option<(u8, u8, u8)>,
}

struct CachedText {
    _texture: GpuTexture,
    bg: Arc<BindGroup>,
    logical_w: f32,
    logical_h: f32,
    origin_x: f32,
    origin_y: f32,
}

pub struct FontSet {
    gpu: Arc<GpuCore>,
    neuropol: SwfFont,
    pricedown: SwfFont,
    helvetica: SwfFont,
    cache: RefCell<FxHashMap<TextMetaKey, FxHashMap<String, CachedText>>>,
}

impl FontSet {
    pub fn new(gpu: Arc<GpuCore>, assets: &AssetBundle) -> Result<Self, String> {
        let find_font = |needle: &str| {
            assets
                .fonts
                .iter()
                .find(|(k, _)| k.contains(needle))
                .map(|(_, f)| f.clone())
                .ok_or_else(|| format!("Missing required SWF font '{}'", needle))
        };
        Ok(Self {
            gpu,
            neuropol: find_font("neuropol")?,
            pricedown: find_font("pricedown")?,
            helvetica: find_font("helvetica")?,
            cache: RefCell::new(FxHashMap::default()),
        })
    }

    #[cfg(test)]
    pub fn cache_entry_count(&self) -> usize {
        self.cache.borrow().values().map(|m| m.len()).sum()
    }

    fn get_swf(&self, fam: FontFamily) -> &SwfFont {
        match fam {
            FontFamily::Neuropol => &self.neuropol,
            FontFamily::Pricedown => &self.pricedown,
            FontFamily::Helvetica => &self.helvetica,
        }
    }

    fn char_advance(&self, fam: FontFamily, ch: char, size: f32, scale_x: f32) -> f32 {
        let swf = self.get_swf(fam);
        let sc = SwfFont::scale_for_size(size) * scale_x;
        swf.glyphs
            .get(&ch)
            .map(|g| g.advance * sc)
            .unwrap_or(size * 0.45 * scale_x)
    }

    pub fn measure_width_scaled(
        &self,
        fam: FontFamily,
        text: &str,
        size: f32,
        scale_x: f32,
    ) -> f32 {
        text.chars()
            .map(|ch| self.char_advance(fam, ch, size, scale_x))
            .sum()
    }

    pub fn measure_width(&self, fam: FontFamily, text: &str, size: f32) -> f32 {
        self.measure_width_scaled(fam, text, size, 1.0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        canvas: &mut Canvas,
        fam: FontFamily,
        x: f32,
        y: f32,
        size: f32,
        color: (u8, u8, u8),
        align: TextAlign,
        text: &str,
    ) {
        self.draw_styled_alpha(
            canvas, fam, x, y, size, 1.0, 1.0, color, None, align, text, 1.0,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_stroked(
        &self,
        canvas: &mut Canvas,
        fam: FontFamily,
        x: f32,
        y: f32,
        size: f32,
        color: (u8, u8, u8),
        stroke: (u8, u8, u8),
        align: TextAlign,
        text: &str,
    ) {
        self.draw_styled_alpha(
            canvas,
            fam,
            x,
            y,
            size,
            1.0,
            1.0,
            color,
            Some(stroke),
            align,
            text,
            1.0,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn get_or_bake(
        &self,
        fam: FontFamily,
        size: f32,
        scale_x: f32,
        scale_y: f32,
        dpr: f32,
        color: (u8, u8, u8),
        stroke: Option<(u8, u8, u8)>,
        text: &str,
    ) -> Option<(Arc<BindGroup>, f32, f32, f32, f32)> {
        let meta = TextMetaKey {
            fam,
            size_q: (size * 10.0).round() as u32,
            sx_q: (scale_x * 100.0).round() as u32,
            sy_q: (scale_y * 100.0).round() as u32,
            dpr_q: (dpr.clamp(1.0, 4.0) * 50.0).round() as u32,
            color,
            stroke,
        };
        let mut cache = self.cache.borrow_mut();
        if !cache.get(&meta).is_some_and(|m| m.contains_key(text)) {
            if cache.len() >= 256 {
                cache.clear();
            }
            let baked = bake_swf_text_gpu(
                &self.gpu,
                self.get_swf(fam),
                size,
                scale_x,
                scale_y,
                dpr,
                color,
                stroke,
                text,
            )?;
            let bg = Arc::new(baked.texture.bind_group.clone());
            let entry = CachedText {
                _texture: baked.texture,
                bg,
                logical_w: baked.logical_w,
                logical_h: baked.logical_h,
                origin_x: baked.origin_x,
                origin_y: baked.origin_y,
            };
            let inner = cache.entry(meta).or_default();
            if inner.len() >= 128 {
                inner.clear();
            }
            inner.insert(text.to_string(), entry);
        }
        cache.get(&meta).and_then(|m| m.get(text)).map(|e| {
            (
                e.bg.clone(),
                e.logical_w,
                e.logical_h,
                e.origin_x,
                e.origin_y,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_fx(
        &self,
        canvas: &mut Canvas,
        fam: FontFamily,
        x: f32,
        y: f32,
        base_size: f32,
        base_sx: f32,
        base_sy: f32,
        extra_sx: f32,
        extra_sy: f32,
        bake_color: (u8, u8, u8),
        stroke: Option<(u8, u8, u8)>,
        align: TextAlign,
        text: &str,
        brightness: f32,
        alpha: f32,
        tint_rgb: [f32; 3],
        blur: (f32, f32),
    ) {
        if alpha <= 0.004 || extra_sx <= 0.004 || extra_sy <= 0.004 || text.is_empty() {
            return;
        }
        let w = self.measure_width_scaled(fam, text, base_size, base_sx) * extra_sx;
        let start_x = match align {
            TextAlign::Left => x,
            TextAlign::Center => x - w * 0.5,
            TextAlign::Right => x - w,
        };
        if let Some((bg, lw, lh, ox, oy)) = self.get_or_bake(
            fam, base_size, base_sx, base_sy, canvas.dpr, bake_color, stroke, text,
        ) {
            canvas.draw_direct_sprite_fx(
                bg,
                [extra_sx, 0.0, start_x - ox * extra_sx, lw],
                [0.0, extra_sy, y - oy * extra_sy, lh],
                brightness,
                alpha,
                tint_rgb,
                blur,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_styled_alpha(
        &self,
        canvas: &mut Canvas,
        fam: FontFamily,
        x: f32,
        y: f32,
        size: f32,
        scale_x: f32,
        scale_y: f32,
        color: (u8, u8, u8),
        stroke: Option<(u8, u8, u8)>,
        align: TextAlign,
        text: &str,
        alpha: f32,
    ) {
        self.draw_fx(
            canvas,
            fam,
            x,
            y,
            size,
            scale_x,
            scale_y,
            1.0,
            1.0,
            color,
            stroke,
            align,
            text,
            1.0,
            alpha,
            [1.0; 3],
            (0.0, 0.0),
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_glow_alpha(
        &self,
        canvas: &mut Canvas,
        fam: FontFamily,
        x: f32,
        y: f32,
        base_size: f32,
        extra_scale: f32,
        align: TextAlign,
        text: &str,
        bake_color: (u8, u8, u8),
        stroke: Option<(u8, u8, u8)>,
        glow_rgb: (u8, u8, u8),
        blur_px: f32,
        alpha: f32,
    ) {
        let tint = [
            glow_rgb.0 as f32 / 255.0,
            glow_rgb.1 as f32 / 255.0,
            glow_rgb.2 as f32 / 255.0,
        ];
        self.draw_fx(
            canvas,
            fam,
            x,
            y,
            base_size,
            1.0,
            1.0,
            extra_scale,
            extra_scale,
            bake_color,
            stroke,
            align,
            text,
            -1.0,
            alpha,
            tint,
            (blur_px, blur_px),
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_rotated_fx(
        &self,
        canvas: &mut Canvas,
        fam: FontFamily,
        cx: f32,
        cy: f32,
        size: f32,
        bake_color: (u8, u8, u8),
        stroke: Option<(u8, u8, u8)>,
        text: &str,
        scale: f32,
        rot_deg: f32,
        brightness: f32,
        alpha: f32,
        tint_rgb: [f32; 3],
        blur: (f32, f32),
    ) {
        if alpha <= 0.004 || scale <= 0.004 || text.is_empty() {
            return;
        }
        let w = self.measure_width_scaled(fam, text, size, 1.0);
        let Some((bg, lw, lh, ox, oy)) =
            self.get_or_bake(fam, size, 1.0, 1.0, canvas.dpr, bake_color, stroke, text)
        else {
            return;
        };
        let (si, co) = rot_deg.to_radians().sin_cos();
        let (a, c, b, d) = (co * scale, -si * scale, si * scale, co * scale);
        let (px, py) = (ox + w * 0.5, oy + size * 0.55);
        canvas.draw_direct_sprite_fx(
            bg,
            [a, c, cx - a * px - c * py, lw],
            [b, d, cy - b * px - d * py, lh],
            brightness,
            alpha,
            tint_rgb,
            blur,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_digits_inner(
        &self,
        canvas: &mut Canvas,
        fam: FontFamily,
        x: f32,
        y: f32,
        base_size: f32,
        base_sx: f32,
        base_sy: f32,
        extra_scale: f32,
        bake_col: (u8, u8, u8),
        stroke: Option<(u8, u8, u8)>,
        align: TextAlign,
        text: &str,
        brightness: f32,
        alpha: f32,
        tint_rgb: [f32; 3],
        blur: (f32, f32),
    ) {
        if alpha <= 0.004 || extra_scale <= 0.004 || text.is_empty() {
            return;
        }
        let total_w = self.measure_width_scaled(fam, text, base_size, base_sx) * extra_scale;
        let mut cur_x = match align {
            TextAlign::Left => x,
            TextAlign::Center => x - total_w * 0.5,
            TextAlign::Right => x - total_w,
        };
        let mut buf = [0u8; 4];
        for ch in text.chars() {
            let adv = self.char_advance(fam, ch, base_size, base_sx) * extra_scale;
            if !ch.is_whitespace() {
                let s = ch.encode_utf8(&mut buf);
                if let Some((bg, lw, lh, ox, oy)) = self.get_or_bake(
                    fam, base_size, base_sx, base_sy, canvas.dpr, bake_col, stroke, s,
                ) {
                    canvas.draw_direct_sprite_fx(
                        bg,
                        [extra_scale, 0.0, cur_x - ox * extra_scale, lw],
                        [0.0, extra_scale, y - oy * extra_scale, lh],
                        brightness,
                        alpha,
                        tint_rgb,
                        blur,
                    );
                }
            }
            cur_x += adv;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_digits_fx(
        &self,
        canvas: &mut Canvas,
        fam: FontFamily,
        x: f32,
        y: f32,
        base_size: f32,
        base_sx: f32,
        base_sy: f32,
        extra_scale: f32,
        color: (u8, u8, u8),
        stroke: Option<(u8, u8, u8)>,
        align: TextAlign,
        text: &str,
        alpha: f32,
        blur: (f32, f32),
    ) {
        let (bake_col, tint_rgb) = if stroke.is_none() {
            (
                (255, 255, 255),
                [
                    color.0 as f32 / 255.0,
                    color.1 as f32 / 255.0,
                    color.2 as f32 / 255.0,
                ],
            )
        } else {
            (color, [1.0, 1.0, 1.0])
        };
        self.draw_digits_inner(
            canvas,
            fam,
            x,
            y,
            base_size,
            base_sx,
            base_sy,
            extra_scale,
            bake_col,
            stroke,
            align,
            text,
            1.0,
            alpha,
            tint_rgb,
            blur,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_digits_glow_alpha(
        &self,
        canvas: &mut Canvas,
        fam: FontFamily,
        x: f32,
        y: f32,
        base_size: f32,
        base_sx: f32,
        base_sy: f32,
        extra_scale: f32,
        bake_color: (u8, u8, u8),
        stroke: Option<(u8, u8, u8)>,
        align: TextAlign,
        text: &str,
        glow_rgb: (u8, u8, u8),
        alpha: f32,
        blur: (f32, f32),
    ) {
        let bake_col = if stroke.is_none() {
            (255, 255, 255)
        } else {
            bake_color
        };
        let tint = [
            glow_rgb.0 as f32 / 255.0,
            glow_rgb.1 as f32 / 255.0,
            glow_rgb.2 as f32 / 255.0,
        ];
        self.draw_digits_inner(
            canvas,
            fam,
            x,
            y,
            base_size,
            base_sx,
            base_sy,
            extra_scale,
            bake_col,
            stroke,
            align,
            text,
            -1.0,
            alpha,
            tint,
            blur,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_multiline(
        &self,
        canvas: &mut Canvas,
        fam: FontFamily,
        x: f32,
        y: f32,
        size: f32,
        line_h: f32,
        color: (u8, u8, u8),
        stroke: Option<(u8, u8, u8)>,
        align: TextAlign,
        max_w: f32,
        text: &str,
        alpha: f32,
    ) -> f32 {
        let lines = self.wrap_lines(fam, text, size, max_w);
        let mut cy = y;
        for ln in &lines {
            self.draw_styled_alpha(
                canvas, fam, x, cy, size, 1.0, 1.0, color, stroke, align, ln, alpha,
            );
            cy += line_h;
        }
        lines.len() as f32 * line_h
    }

    pub fn wrap_lines(&self, fam: FontFamily, text: &str, size: f32, max_w: f32) -> Vec<String> {
        let mut out = Vec::new();
        for para in text.split('\n') {
            let trimmed = para.trim();
            if trimmed.is_empty() {
                out.push(String::new());
                continue;
            }
            let mut cur = String::new();
            for word in trimmed.split_whitespace() {
                let candidate = if cur.is_empty() {
                    word.to_string()
                } else {
                    format!("{} {}", cur, word)
                };
                if self.measure_width(fam, &candidate, size) <= max_w || cur.is_empty() {
                    cur = candidate;
                } else {
                    out.push(cur);
                    cur = word.to_string();
                }
            }
            if !cur.is_empty() {
                out.push(cur);
            }
        }
        if out.is_empty() {
            out.push(String::new());
        }
        out
    }
}
