// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::AssetBundle;
use crate::render::bg_sprites::draw_popup_sprite;
use crate::render::canvas::{Canvas, LOGICAL_H, LOGICAL_W, Rect};
use crate::render::tex_bank::TexBank;

struct StarDef {
    x: f32,
    y: f32,
    base_scale: f32,
    base_alpha: f32,
    phase: f32,
    speed: f32,
}

pub struct BgAnimState {
    pub bg_id: String,
    pub age: f32,
    pub darken: f32,
    pub target_darken: f32,
    pub darken_delay: f32,
}

impl BgAnimState {
    pub fn new(bg_id: &str) -> Self {
        Self {
            bg_id: bg_id.to_string(),
            age: 0.0,
            darken: 0.0,
            target_darken: 0.0,
            darken_delay: 0.0,
        }
    }

    pub fn load(&mut self, bg_id: &str) {
        let norm = if bg_id.eq_ignore_ascii_case("bluesky") {
            "DaySky"
        } else {
            bg_id
        };
        if self.bg_id != norm {
            self.bg_id = norm.to_string();
            self.age = 0.0;
        }
        self.darken = 0.0;
        self.target_darken = 0.0;
        self.darken_delay = 0.0;
    }

    pub fn darken(&mut self, delay: f32) {
        self.target_darken = 1.0;
        self.darken_delay = 1.0 + delay;
    }

    pub fn lighten(&mut self, delay: f32) {
        self.target_darken = 0.0;
        self.darken_delay = 1.0 + delay;
    }

    pub fn update(&mut self, dt: f32) {
        self.age += dt;
        if self.darken_delay > 0.0 {
            self.darken_delay = (self.darken_delay - dt).max(0.0);
        } else if (self.darken - self.target_darken).abs() > 0.001 {
            let step = dt / 1.6;
            if self.target_darken > self.darken {
                self.darken = (self.darken + step).min(1.0);
            } else {
                self.darken = (self.darken - step).max(0.0);
            }
        }
    }
}

pub struct BackgroundRenderer {
    stars: Vec<StarDef>,
}

impl BackgroundRenderer {
    pub fn new() -> Self {
        let mut stars = Vec::with_capacity(18);
        let mut seed: u32 = 0x517CC1B7;
        let mut next_f = || -> f32 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 16) as f32 / 65535.0
        };
        for _ in 0..18 {
            let x = 25.0 + next_f() * 750.0;
            let y = 25.0 + next_f() * 400.0;
            let inv_y = 1.0 - y / 400.0;
            let base_alpha = (inv_y * (0.9 - 0.5) + 0.25).clamp(0.25, 0.9);
            let base_scale = (next_f() + 0.2).min(1.0);
            let phase = next_f() * std::f32::consts::TAU;
            let speed = 1.5 + next_f() * 2.0;
            stars.push(StarDef {
                x,
                y,
                base_scale,
                base_alpha,
                phase,
                speed,
            });
        }
        Self { stars }
    }

    pub fn draw_background(
        &self,
        canvas: &mut Canvas,
        assets: &AssetBundle,
        bank: &TexBank,
        anim: &BgAnimState,
    ) {
        let full = Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32);
        match anim.bg_id.as_str() {
            "DaySky" | "bluesky" => {
                self.draw_full_bitmap(canvas, bank, "DaySkyBitmap", 1.0);
                if anim.darken > 0.01 {
                    canvas.fill_rect_alpha(full, 0, 0, 0, (anim.darken * 0.3 * 255.0) as u8);
                }
            }
            "OuterSpace" => {
                self.draw_full_bitmap(canvas, bank, "SpaceSkyBitmap", 1.0);
                if anim.darken > 0.01 {
                    canvas.fill_rect_alpha(full, 0, 0, 0, (anim.darken * 0.3 * 255.0) as u8);
                }
            }
            "SnowyHills" => self.draw_snowy_hills(canvas, assets, bank, anim),
            _ => {
                self.draw_night_hills_rear(canvas, assets, bank, anim);
                self.draw_night_hills_front(canvas, assets, bank, anim);
            }
        }
    }

    pub fn draw_night_hills_rear(
        &self,
        canvas: &mut Canvas,
        assets: &AssetBundle,
        bank: &TexBank,
        anim: &BgAnimState,
    ) {
        let wash_alpha = ((anim.age - 0.7) / 1.5).clamp(0.0, 1.0);
        if wash_alpha < 0.999 {
            canvas.clear((0, 0, 0));
        }
        if wash_alpha > 0.004 {
            self.draw_full_bitmap(canvas, bank, "CombinedBgBitmap", wash_alpha);
        }
        self.draw_stars(canvas, anim.age);
        if let Some(items) = assets.sprites.get("TitleBgAssetsSprite") {
            draw_popup_sprite(canvas, bank, items, anim, false, |layer| {
                !matches!(layer, Some("frontHillsLayer") | Some("frontCloudsLayer"))
            });
        }
    }

    pub fn draw_night_hills_front(
        &self,
        canvas: &mut Canvas,
        assets: &AssetBundle,
        bank: &TexBank,
        anim: &BgAnimState,
    ) {
        if let Some(items) = assets.sprites.get("TitleBgAssetsSprite") {
            draw_popup_sprite(canvas, bank, items, anim, false, |layer| {
                matches!(layer, Some("frontHillsLayer") | Some("frontCloudsLayer"))
            });
        }
    }

    fn draw_snowy_hills(
        &self,
        canvas: &mut Canvas,
        assets: &AssetBundle,
        bank: &TexBank,
        anim: &BgAnimState,
    ) {
        let sky_alpha = (anim.age / 1.3).clamp(0.0, 1.0);
        if sky_alpha < 0.999 {
            canvas.clear((0, 0, 0));
        }
        if sky_alpha > 0.004 {
            self.draw_full_bitmap(canvas, bank, "SnowySkyBitmap", sky_alpha);
        }
        if let Some(items) = assets.sprites.get("SnowHillsBgAssetsSprite") {
            draw_popup_sprite(canvas, bank, items, anim, true, |_| true);
        }
    }

    fn draw_full_bitmap(&self, canvas: &mut Canvas, bank: &TexBank, key: &str, alpha: f32) {
        if let Some((tid, _, _)) = bank.named(key) {
            canvas.draw_tex_rect(
                tid,
                Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
                None,
                1.0,
                alpha,
            );
        }
    }

    fn draw_stars(&self, canvas: &mut Canvas, time: f32) {
        for s in &self.stars {
            let pulse = 0.2 + 0.8 * (0.5 + 0.5 * (time * s.speed + s.phase).sin());
            let rad = (s.base_scale * pulse * 4.2).max(1.2);
            let opacity = (s.base_alpha * pulse).clamp(0.0, 1.0);
            canvas.draw_star_glow(s.x, s.y, rad, opacity);
        }
    }
}
