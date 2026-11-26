// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::render::easing::back_out;
use crate::render::{
    Canvas, FontFamily, FontSet, Rect, StarField, TexBank, TextAlign, TextOutline,
};
use crate::ui::widgets::{ScoreRoller, UiAction, UiButton, draw_text_button};
use rand::RngExt;

pub struct ChampionState {
    pub age: f32,
    pub star_field: StarField,
    pub score_roller: ScoreRoller,
    pub show_trophy_tip: bool,
    pub hiding: Option<(f32, bool)>,
}

impl ChampionState {
    pub fn new(final_score: u32, has_ultralux: bool, rng: &mut impl RngExt) -> Self {
        let mut score_roller = ScoreRoller::new(0);
        score_roller.roll_to(final_score, 3.0, 5.6);
        Self {
            age: 0.0,
            star_field: StarField::new(rng),
            score_roller,
            show_trophy_tip: !has_ultralux,
            hiding: None,
        }
    }

    pub fn start_hide(&mut self, quit: bool) {
        if self.hiding.is_none() {
            self.hiding = Some((0.0, quit));
        }
    }

    pub fn update(&mut self, dt: f32) -> Option<bool> {
        self.age += dt;
        self.star_field.update(dt);
        self.score_roller.update(dt);
        if let Some((mut h, quit)) = self.hiding.take() {
            h += dt;
            if h >= 1.0 {
                return Some(quit);
            }
            self.hiding = Some((h, quit));
        }
        None
    }
}

pub fn build_champion_buttons(champ: &ChampionState) -> Vec<UiButton> {
    if champ.hiding.is_some() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if champ.age >= 8.0 {
        out.push(UiButton {
            id: "ch_menu",
            rect: Rect::new(220.0, 526.0, 230.0, 40.0),
            action: UiAction::DirectMainMenu,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        });
    }
    if champ.age >= 8.3 {
        out.push(UiButton {
            id: "ch_quit",
            rect: Rect::new(470.0, 526.0, 120.0, 40.0),
            action: UiAction::DirectQuit,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        });
    }
    out
}

pub fn render_champion_screen(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    champ: &ChampionState,
    mouse_pos: (f32, f32),
) {
    let hide_a = champ
        .hiding
        .map(|(h, _)| (1.0 - h).clamp(0.0, 1.0))
        .unwrap_or(1.0);
    let sf_alpha = champ.age.clamp(0.0, 1.0) * hide_a;
    champ.star_field.draw(canvas, bank, mouse_pos, sf_alpha);

    let px = ((mouse_pos.0 - 400.0) / 400.0 * -20.0).round();
    let py = ((mouse_pos.1 - 300.0) / 300.0 * -20.0).round();
    let glow_p = 0.5 - 0.5 * (champ.age * std::f32::consts::PI / 0.8).cos();
    let c_glow_blur = 32.0 * glow_p;
    let bright = 1.0 + 0.8 * glow_p;

    if champ.age >= 3.0 {
        let u = ((champ.age - 3.0) / 0.55).clamp(0.0, 1.0);
        let b = back_out(u);
        let sc = (0.1 + 0.9 * b).max(0.01);
        let cy = 250.0 - 100.0 * b + py;
        let rot = 35.0 * (1.0 - b);
        let bl = (1.0 - u) * 64.0;
        if let Some((tid, lw, lh)) = bank.named("GreenTitleBitmap") {
            let w = lw as f32 * sc;
            let h = lh as f32 * sc;
            if glow_p > 0.01 {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    [0.0, 0.0, 1.0, 1.0],
                    400.0 + px,
                    cy,
                    w,
                    h,
                    rot,
                    None,
                    -1.0,
                    glow_p * u * hide_a,
                    false,
                    (bl + c_glow_blur, bl + c_glow_blur),
                );
            }
            canvas.draw_tex_uv_rotated_blur(
                tid,
                [0.0, 0.0, 1.0, 1.0],
                400.0 + px,
                cy,
                w,
                h,
                rot,
                None,
                1.0,
                u * hide_a,
                false,
                (bl, bl),
            );
        }
    }

    let title_outline = Some(TextOutline::double((0, 0, 0), (0xF4, 0xF2, 0x8A)));
    for (start, target_sx, label) in [(3.8f32, 45.0f32, "GRAND"), (4.6, 210.0, "CHAMPION!")] {
        if champ.age >= start {
            let u = ((champ.age - start) / 0.55).clamp(0.0, 1.0);
            let b = back_out(u);
            let sc = (0.1 + 0.9 * b.max(0.0)).max(0.05);
            let bl = (1.0 - u) * 64.0;
            let w = fonts.measure_width(FontFamily::Neuropol, label, 46.0);
            let sx = 400.0 + (target_sx - 400.0) * b + px;
            let sy = 300.0 - 50.0 * b + py;
            let gx = sx + (w * 0.5) * sc;
            let gy = sy + 26.0 * sc;
            if glow_p > 0.01 {
                fonts.draw_glow_alpha(
                    canvas,
                    FontFamily::Neuropol,
                    gx,
                    gy,
                    46.0,
                    sc,
                    TextAlign::Left,
                    label,
                    (0x56, 0xA0, 0xA9),
                    title_outline,
                    (255, 255, 255),
                    bl + c_glow_blur,
                    glow_p * u * hide_a,
                );
            }
            fonts.draw_fx(
                canvas,
                FontFamily::Neuropol,
                gx,
                gy,
                46.0,
                1.0,
                1.0,
                sc,
                sc,
                (0x56, 0xA0, 0xA9),
                title_outline,
                TextAlign::Left,
                label,
                bright,
                u * hide_a,
                [1.0; 3],
                (bl, bl),
            );
        }
    }

    if champ.age >= 5.2 {
        let lu = ((champ.age - 5.2) / 0.7).clamp(0.0, 1.0);
        let lbl = (1.0 - lu) * 64.0;
        if glow_p > 0.01 {
            fonts.draw_fx(
                canvas,
                FontFamily::Neuropol,
                400.0 + px,
                384.0 + py,
                32.0,
                1.0,
                1.0,
                1.0,
                1.0,
                (255, 255, 255),
                Some(TextOutline::stroke((0, 0, 0))),
                TextAlign::Center,
                "Final Score:",
                -1.0,
                glow_p * lu * hide_a,
                [1.0; 3],
                (lbl + c_glow_blur, c_glow_blur),
            );
        }
        fonts.draw_fx(
            canvas,
            FontFamily::Neuropol,
            400.0 + px,
            384.0 + py,
            32.0,
            1.0,
            1.0,
            1.0,
            1.0,
            (255, 255, 255),
            Some(TextOutline::stroke((0, 0, 0))),
            TextAlign::Center,
            "Final Score:",
            1.0,
            lu * hide_a,
            [1.0; 3],
            (lbl, 0.0),
        );
    }
    if champ.age >= 5.6 {
        let su = ((champ.age - 5.6) / 0.7).clamp(0.0, 1.0);
        let sbl = (1.0 - su) * 64.0;
        let txt = champ.score_roller.value().to_string();
        if glow_p > 0.01 {
            fonts.draw_digits_glow_alpha(
                canvas,
                FontFamily::Neuropol,
                400.0 + px,
                430.0 + py,
                28.0,
                1.0,
                1.0,
                1.0,
                (0xEA, 0xEA, 0xEA),
                Some(TextOutline::shadowed((0x00, 0x00, 0x40))),
                TextAlign::Center,
                &txt,
                (255, 255, 255),
                glow_p * su * hide_a,
                (sbl + c_glow_blur, c_glow_blur),
            );
        }
        fonts.draw_digits_fx(
            canvas,
            FontFamily::Neuropol,
            400.0 + px,
            430.0 + py,
            28.0,
            1.0,
            1.0,
            1.0,
            (0xEA, 0xEA, 0xEA),
            Some(TextOutline::shadowed((0x00, 0x00, 0x40))),
            TextAlign::Center,
            &txt,
            su * hide_a,
            (sbl, 0.0),
        );
    }

    if champ.age >= 7.0 && champ.show_trophy_tip {
        let tu = (champ.age - 7.0).clamp(0.0, 1.0) * hide_a;
        fonts.draw_styled_alpha(
            canvas,
            FontFamily::Neuropol,
            400.0,
            570.0,
            20.0,
            1.0,
            1.0,
            (255, 255, 255),
            None,
            TextAlign::Center,
            "Tip: Retry levels from the Trophy Room to earn platinum trophies!",
            tu,
        );
    }

    let allow_hov = champ.hiding.is_none();
    if champ.age >= 8.0 {
        let ma = ((champ.age - 8.0) / 2.0).clamp(0.0, 1.0) * hide_a;
        draw_text_button(
            canvas,
            fonts,
            220.0,
            530.0,
            36.0,
            TextAlign::Left,
            "Main Menu",
            allow_hov && Rect::new(220.0, 526.0, 230.0, 40.0).contains(mouse_pos.0, mouse_pos.1),
            None,
            ma,
        );
    }
    if champ.age >= 8.3 {
        let qa = ((champ.age - 8.3) / 2.0).clamp(0.0, 1.0) * hide_a;
        draw_text_button(
            canvas,
            fonts,
            470.0,
            530.0,
            36.0,
            TextAlign::Left,
            "Quit",
            allow_hov && Rect::new(470.0, 526.0, 120.0, 40.0).contains(mouse_pos.0, mouse_pos.1),
            None,
            qa,
        );
    }
}
