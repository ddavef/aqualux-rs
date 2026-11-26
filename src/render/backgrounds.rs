// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::AssetBundle;
use crate::render::bg_sprites::draw_popup_sprite;
use crate::render::canvas::{Canvas, LOGICAL_H, LOGICAL_W, Rect};
use crate::render::easing::quad_out;
use crate::render::tex_bank::TexBank;
use rand::RngExt;
use rand::rngs::StdRng;

const STAR_COUNT: usize = 18;
const STAR_RECT_X: f32 = 25.0;
const STAR_RECT_Y: f32 = 25.0;
const STAR_RECT_W: f32 = 750.0;
const STAR_RECT_H: f32 = 400.0;
const STAR_MIN_SIZE: f32 = 4.0;
const STAR_MAX_SIZE: f32 = 16.0;
const STAR_MAX_ALPHA: f32 = 0.9;
const STAR_MIN_ALPHA: f32 = 0.5;
const STAR_TICK_SECS: f32 = 0.075;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NightStar {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub alpha: f32,
    pub scales: [f32; 18],
}

impl NightStar {
    /// Precomputes the exact 75ms `StarSprite.updateSparkle()` scale timeline in IEEE-754 `f64`:
    /// ticks `0..6` settle into a 12-tick (`0.90s`) cycle stored in `scales[6..18]`.
    pub(crate) fn build_scales(mut value: f64, init_scale: f32) -> [f32; 18] {
        let mut scales = [0.0f32; 18];
        scales[0] = init_scale;
        let mut is_growing = true;
        for slot in &mut scales[1..] {
            if is_growing {
                value += 0.3;
            } else {
                value -= 0.3;
            }
            let sx = value.clamp(0.2, 1.0);
            if value >= 2.0 {
                value = 2.0;
                is_growing = false;
            }
            if sx <= 0.2 {
                is_growing = true;
            }
            *slot = sx as f32;
        }
        scales
    }

    pub(crate) fn scale_at_age(&self, age: f32) -> f32 {
        let tick = (age.max(0.0) / STAR_TICK_SECS).floor() as usize;
        if tick < 6 {
            self.scales[tick]
        } else {
            self.scales[6 + (tick - 6) % 12]
        }
    }

    pub(crate) fn generate_layer(rng: &mut impl RngExt) -> Vec<Self> {
        (0..STAR_COUNT)
            .map(|_| {
                let x = (STAR_RECT_X + rng.random_range(0.0..1.0) * STAR_RECT_W).floor();
                let y = (STAR_RECT_Y + rng.random_range(0.0..1.0) * STAR_RECT_H).floor();
                let radius = (rng.random_range(0.0..1.0) * (STAR_MAX_SIZE + STAR_MIN_SIZE)
                    - STAR_MIN_SIZE)
                    .floor()
                    .abs();
                let init_value = ((rng.random_range(0.0f64..1.0) * 18.0).floor() + 2.0) / 10.0;
                let dist_from_bottom = 1.0 - y / STAR_RECT_H;
                let alpha = (dist_from_bottom * (STAR_MAX_ALPHA - STAR_MIN_ALPHA)
                    + STAR_MIN_ALPHA * 0.5)
                    .clamp(0.0, 1.0);
                let init_scale = (rng.random_range(0.0f32..1.0) + 0.2).min(1.0);
                Self {
                    x,
                    y,
                    radius,
                    alpha,
                    scales: Self::build_scales(init_value, init_scale),
                }
            })
            .collect()
    }
}

const CROSSFADE_SECS: f32 = 1.0;

#[derive(Clone)]
pub(crate) struct OutgoingBg {
    pub state: BgAnimState,
    pub fade_elapsed: f32,
}

#[derive(Clone)]
pub struct BgAnimState {
    pub bg_id: String,
    pub age: f32,
    pub darken: f32,
    pub target_darken: f32,
    pub darken_delay: f32,
    darken_from: f32,
    darken_elapsed: f32,
    darken_active: bool,
    pub(crate) stars: Vec<NightStar>,
    pub(crate) outgoing: Option<Box<OutgoingBg>>,
}

fn normalize_bg_id(bg_id: &str) -> &str {
    match bg_id.trim().to_ascii_lowercase().as_str() {
        "nighthills" => "NightHills",
        "daysky" | "bluesky" => "DaySky",
        "snowyhills" | "snowhills" => "SnowyHills",
        "outerspace" | "spacesky" => "OuterSpace",
        _ => bg_id,
    }
}

impl BgAnimState {
    pub fn new(bg_id: &str) -> Self {
        let mut rng: StdRng = rand::make_rng();
        let norm = normalize_bg_id(bg_id);
        Self {
            bg_id: norm.to_string(),
            age: 0.0,
            darken: 0.0,
            target_darken: 0.0,
            darken_delay: 0.0,
            darken_from: 0.0,
            darken_elapsed: 0.0,
            darken_active: false,
            stars: NightStar::generate_layer(&mut rng),
            outgoing: None,
        }
    }

    pub fn load(&mut self, bg_id: &str) {
        let norm = normalize_bg_id(bg_id);
        if self.bg_id == norm {
            return;
        }
        let mut prev = self.clone();
        prev.outgoing = None;
        self.outgoing = Some(Box::new(OutgoingBg {
            state: prev,
            fade_elapsed: 0.0,
        }));
        self.bg_id = norm.to_string();
        self.age = 0.0;
        self.darken = 0.0;
        self.target_darken = 0.0;
        self.darken_delay = 0.0;
        self.darken_from = 0.0;
        self.darken_elapsed = 0.0;
        self.darken_active = false;
        if norm == "NightHills" {
            let mut rng: StdRng = rand::make_rng();
            self.stars = NightStar::generate_layer(&mut rng);
        }
    }

    pub fn darken(&mut self, delay: f32) {
        self.darken_from = self.darken;
        self.target_darken = 1.0;
        self.darken_delay = 1.0 + delay;
        self.darken_elapsed = 0.0;
        self.darken_active = true;
    }

    pub fn lighten(&mut self, delay: f32) {
        if delay > 0.0 {
            self.darken = 1.0;
        }
        if self.darken > 0.001 || (self.darken_active && self.target_darken > 0.5) {
            self.darken_from = self.darken;
            self.target_darken = 0.0;
            self.darken_delay = 1.0 + delay;
            self.darken_elapsed = 0.0;
            self.darken_active = true;
        } else {
            self.target_darken = 0.0;
            self.darken_delay = 0.0;
            self.darken_elapsed = 0.0;
            self.darken_active = false;
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.age += dt;
        if self.darken_active {
            let mut step = dt;
            if self.darken_delay > 0.0 {
                if step <= self.darken_delay {
                    self.darken_delay -= step;
                    step = 0.0;
                } else {
                    step -= self.darken_delay;
                    self.darken_delay = 0.0;
                }
            }
            if step > 0.0 {
                self.darken_elapsed = (self.darken_elapsed + step).min(1.6);
                let e = quad_out((self.darken_elapsed / 1.6).clamp(0.0, 1.0));
                self.darken = self.darken_from + (self.target_darken - self.darken_from) * e;
                if self.darken_elapsed >= 1.6 {
                    self.darken = self.target_darken;
                    self.darken_active = false;
                }
            }
        }
        if let Some(out) = &mut self.outgoing {
            out.state.update(dt);
            out.fade_elapsed += dt;
            if out.fade_elapsed >= CROSSFADE_SECS {
                self.outgoing = None;
            }
        }
    }
}

pub struct BackgroundRenderer;

impl BackgroundRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn draw_background(
        &self,
        canvas: &mut Canvas,
        assets: &AssetBundle,
        bank: &TexBank,
        anim: &BgAnimState,
    ) {
        self.draw_single_background(canvas, assets, bank, anim, 1.0, false);
        self.draw_outgoing_overlay(canvas, assets, bank, anim);
    }

    pub fn draw_outgoing_overlay(
        &self,
        canvas: &mut Canvas,
        assets: &AssetBundle,
        bank: &TexBank,
        anim: &BgAnimState,
    ) {
        if let Some(out) = &anim.outgoing {
            let out_alpha = 1.0 - quad_out((out.fade_elapsed / CROSSFADE_SECS).clamp(0.0, 1.0));
            if out_alpha > 0.004 {
                self.draw_single_background(canvas, assets, bank, &out.state, out_alpha, true);
            }
        }
    }

    fn draw_single_background(
        &self,
        canvas: &mut Canvas,
        assets: &AssetBundle,
        bank: &TexBank,
        anim: &BgAnimState,
        layer_alpha: f32,
        is_overlay: bool,
    ) {
        let a = layer_alpha.clamp(0.0, 1.0);
        if a <= 0.004 {
            return;
        }
        let full = Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32);
        match anim.bg_id.as_str() {
            "DaySky" | "bluesky" => {
                self.draw_full_bitmap(canvas, bank, "DaySkyBitmap", a);
                if anim.darken > 0.01 {
                    canvas.fill_rect_alpha(full, 0, 0, 0, (anim.darken * 0.3 * a * 255.0) as u8);
                }
            }
            "OuterSpace" => {
                self.draw_full_bitmap(canvas, bank, "SpaceSkyBitmap", a);
                if anim.darken > 0.01 {
                    canvas.fill_rect_alpha(full, 0, 0, 0, (anim.darken * 0.3 * a * 255.0) as u8);
                }
            }
            "SnowyHills" => self.draw_snowy_hills(canvas, assets, bank, anim, a, is_overlay),
            _ => {
                self.draw_night_hills_rear_alpha(canvas, assets, bank, anim, a, is_overlay);
                self.draw_night_hills_front_alpha(canvas, assets, bank, anim, a);
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
        self.draw_night_hills_rear_alpha(canvas, assets, bank, anim, 1.0, false);
    }

    fn draw_night_hills_rear_alpha(
        &self,
        canvas: &mut Canvas,
        assets: &AssetBundle,
        bank: &TexBank,
        anim: &BgAnimState,
        layer_alpha: f32,
        is_overlay: bool,
    ) {
        let wash_alpha = quad_out(((anim.age - 0.7) / 1.5).clamp(0.0, 1.0));
        if wash_alpha < 0.999 {
            if !is_overlay && layer_alpha >= 0.999 {
                canvas.clear((0, 0, 0));
            } else {
                let full = Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32);
                canvas.fill_rect_alpha(
                    full,
                    0,
                    0,
                    0,
                    ((1.0 - wash_alpha) * layer_alpha * 255.0).round() as u8,
                );
            }
        }
        if wash_alpha * layer_alpha > 0.004 {
            self.draw_full_bitmap(canvas, bank, "CombinedBgBitmap", wash_alpha * layer_alpha);
        }
        self.draw_stars(canvas, anim, layer_alpha);
        if let Some(items) = assets.sprites.get("TitleBgAssetsSprite") {
            draw_popup_sprite(canvas, bank, items, anim, false, layer_alpha, |layer| {
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
        self.draw_night_hills_front_alpha(canvas, assets, bank, anim, 1.0);
    }

    fn draw_night_hills_front_alpha(
        &self,
        canvas: &mut Canvas,
        assets: &AssetBundle,
        bank: &TexBank,
        anim: &BgAnimState,
        layer_alpha: f32,
    ) {
        if let Some(items) = assets.sprites.get("TitleBgAssetsSprite") {
            draw_popup_sprite(canvas, bank, items, anim, false, layer_alpha, |layer| {
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
        layer_alpha: f32,
        is_overlay: bool,
    ) {
        let sky_alpha = quad_out((anim.age / 1.3).clamp(0.0, 1.0));
        if sky_alpha < 0.999 {
            if !is_overlay && layer_alpha >= 0.999 {
                canvas.clear((0, 0, 0));
            } else {
                let full = Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32);
                canvas.fill_rect_alpha(
                    full,
                    0,
                    0,
                    0,
                    ((1.0 - sky_alpha) * layer_alpha * 255.0).round() as u8,
                );
            }
        }
        if sky_alpha * layer_alpha > 0.004 {
            self.draw_full_bitmap(canvas, bank, "SnowySkyBitmap", sky_alpha * layer_alpha);
        }
        if let Some(items) = assets.sprites.get("SnowHillsBgAssetsSprite") {
            draw_popup_sprite(canvas, bank, items, anim, true, layer_alpha, |_| true);
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

    fn draw_stars(&self, canvas: &mut Canvas, anim: &BgAnimState, layer_alpha: f32) {
        for s in &anim.stars {
            let rad = s.radius * s.scale_at_age(anim.age);
            if rad > 0.0 {
                canvas.draw_star_glow(s.x, s.y, rad, s.alpha * layer_alpha);
            }
        }
    }
}
