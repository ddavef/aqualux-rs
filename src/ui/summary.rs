// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::{MetricResult, Trophy};
use crate::render::easing::{back_in, back_out, bounce_out, strong_in, strong_out};
use crate::render::{
    Canvas, FontFamily, FontSet, GlowPreset, Rect, TexBank, TextAlign, draw_rainbow,
};
use crate::ui::widgets::{ScoreRoller, UiAction, UiButton, draw_text_button};

const CENTER_X: f32 = 415.0;
const LABEL_RIGHT: f32 = 410.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryHideAction {
    NextLevel,
    RetryLevel,
    MainMenu,
}

pub struct SummaryState {
    pub result: MetricResult,
    pub age: f32,
    pub fill_roller: ScoreRoller,
    pub move_roller: ScoreRoller,
    pub time_roller: ScoreRoller,
    pub total_roller: ScoreRoller,
    pub fire_accum: f32,
    pub hiding: Option<(f32, SummaryHideAction)>,
}

impl SummaryState {
    pub fn new(res: MetricResult, old_total: u32, new_total: u32) -> Self {
        let mut fill_roller = ScoreRoller::new(0);
        fill_roller.roll_to(res.pipe_bonus, 3.0, 1.1);
        let mut move_roller = ScoreRoller::new(0);
        move_roller.roll_to(res.move_bonus, 3.0, 1.2);
        let mut time_roller = ScoreRoller::new(0);
        time_roller.roll_to(res.time_bonus, 3.0, 1.3);
        let mut total_roller = ScoreRoller::new(old_total);
        total_roller.roll_to(new_total, 3.0, 1.4);

        Self {
            result: res,
            age: 0.0,
            fill_roller,
            move_roller,
            time_roller,
            total_roller,
            fire_accum: 0.0,
            hiding: None,
        }
    }

    pub fn start_hide(&mut self, action: SummaryHideAction) {
        if self.hiding.is_none() {
            self.hiding = Some((0.0, action));
        }
    }

    pub fn update(&mut self, dt: f32) -> (bool, Option<SummaryHideAction>) {
        if let Some((mut h, act)) = self.hiding.take() {
            h += dt;
            if h >= 2.0 {
                return (false, Some(act));
            }
            self.hiding = Some((h, act));
            return (false, None);
        }
        self.age += dt;
        self.fill_roller.update(dt);
        self.move_roller.update(dt);
        self.time_roller.update(dt);
        self.total_roller.update(dt);
        if self.age >= 1.0 {
            self.fire_accum += dt;
            if self.fire_accum >= 0.1 {
                self.fire_accum -= 0.1;
                return (true, None);
            }
        }
        (false, None)
    }
}

pub fn build_summary_buttons(summary: &SummaryState) -> Vec<UiButton> {
    if summary.hiding.is_some() || summary.age < 3.5 {
        return Vec::new();
    }
    let mut out = vec![
        UiButton {
            id: "sum_cont",
            rect: Rect::new(255.0, 494.0, 320.0, 38.0),
            action: UiAction::NextLevel,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "sum_retry",
            rect: Rect::new(305.0, 535.0, 220.0, 28.0),
            action: UiAction::RetryLevel,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "sum_menu",
            rect: Rect::new(305.0, 564.0, 220.0, 28.0),
            action: UiAction::MainMenuConfirmFromSummary,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
    ];
    if summary.result.trophy == Trophy::None {
        out.push(UiButton {
            id: "sum_trophy",
            rect: Rect::new(600.0, 320.0, 100.0, 100.0),
            action: UiAction::RetryLevel,
            enabled: true,
            hover_sfx: None,
        });
    }
    out
}

pub fn render_summary_screen(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    summary: &SummaryState,
    mouse_pos: (f32, f32),
) {
    let hide_t = summary.hiding.map(|(h, _)| h);
    let fade_late = hide_t
        .map(|h| (1.0 - (h - 0.5).max(0.0) / 1.5).clamp(0.0, 1.0))
        .unwrap_or(1.0);
    let rainbow_p = (summary.age / 0.9).clamp(0.0, 1.0);
    draw_rainbow(canvas, bank, rainbow_p, fade_late);

    if summary.age >= 0.8 {
        let u = ((summary.age - 0.8) / 0.9).clamp(0.0, 1.0);
        let b = back_out(u);
        let sc = (0.1 + 0.9 * b).max(0.01);
        let rot = -500.0 * (1.0 - b);
        let wb = (1.0 - u) * 8.0;
        let col = (0x40, 0xD1, 0xAF);
        let st = Some((0x00, 0x00, 0x40));
        fonts.draw_rotated_fx(
            canvas,
            FontFamily::Pricedown,
            620.0,
            140.0,
            72.0,
            col,
            st,
            "well done!",
            sc,
            rot,
            -1.0,
            0.75 * u * fade_late,
            [1.0, 1.0, 176.0 / 255.0],
            (22.0 + wb, 22.0 + wb),
        );
        fonts.draw_rotated_fx(
            canvas,
            FontFamily::Pricedown,
            620.0,
            140.0,
            72.0,
            col,
            st,
            "well done!",
            sc,
            rot,
            1.0,
            u * fade_late,
            [1.0; 3],
            (wb, wb),
        );
    }

    let rows = [
        ("fill score:", summary.fill_roller.value(), 280.0, 1.1, 18.0),
        (
            "moves score:",
            summary.move_roller.value(),
            320.0,
            1.2,
            18.0,
        ),
        ("time score:", summary.time_roller.value(), 360.0, 1.3, 18.0),
        ("Total:", summary.total_roller.value(), 420.0, 1.4, 21.0),
    ];
    for (idx, (lbl, val, y, delay, val_sz)) in rows.into_iter().enumerate() {
        if summary.age >= delay {
            let lw = fonts.measure_width(FontFamily::Neuropol, lbl, 32.0);
            let u = ((summary.age - delay) / 0.8).clamp(0.0, 1.0);
            let lx = if let Some(h) = hide_t {
                let hu = ((h - 0.1 * (idx + 1) as f32) / 0.9).clamp(0.0, 1.0);
                LABEL_RIGHT + (800.0 + lw + 20.0 - LABEL_RIGHT) * back_in(hu)
            } else {
                -10.0 + (LABEL_RIGHT + 10.0) * back_out(u)
            };
            let lblur = if hide_t.is_some() {
                0.0
            } else {
                (1.0 - u) * 16.0
            };
            if lx - lw <= 815.0 {
                fonts.draw_fx(
                    canvas,
                    FontFamily::Neuropol,
                    lx,
                    y,
                    32.0,
                    1.0,
                    1.0,
                    1.0,
                    1.0,
                    (0xEA, 0xEA, 0xEA),
                    Some((0x00, 0x00, 0x40)),
                    TextAlign::Right,
                    lbl,
                    1.0,
                    1.0,
                    [1.0; 3],
                    (lblur, 0.0),
                );
            }

            let nu = ((summary.age - (delay + 0.2)) / 0.8).clamp(0.0, 1.0);
            if nu > 0.0 {
                let nx = 800.0 + (LABEL_RIGHT + 14.0 - 800.0) * strong_out(nu);
                let na = hide_t
                    .map(|h| (1.0 - (h - (0.5 + 0.1 * idx as f32)).max(0.0)).clamp(0.0, 1.0))
                    .unwrap_or(1.0);
                fonts.draw_digits_fx(
                    canvas,
                    FontFamily::Neuropol,
                    nx,
                    y + (32.0 - val_sz) * 0.65,
                    val_sz,
                    1.0,
                    1.0,
                    1.0,
                    (0xEA, 0xEA, 0xEA),
                    Some((0x00, 0x00, 0x40)),
                    TextAlign::Left,
                    &val.to_string(),
                    na,
                    ((1.0 - nu) * 16.0, 0.0),
                );
            }
        }
    }

    if summary.age >= 2.3 {
        let tu = ((summary.age - 2.3) / 0.85).clamp(0.0, 1.0);
        let ta = tu * fade_late;
        let t_blur = ((1.0 - tu) * 8.0, (1.0 - tu) * 8.0);
        let sc = 0.1 + 1.7 * bounce_out(tu);
        let sz = 100.0 * sc;
        let rect = Rect::new(650.0 - sz * 0.5, 370.0 - sz * 0.5, sz, sz);
        let key = summary.result.trophy.bitmap_name();
        if let Some((tid, bw, bh)) = bank.named(key) {
            if summary.hiding.is_none()
                && summary.result.trophy == Trophy::None
                && rect.contains(mouse_pos.0, mouse_pos.1)
                && let Some((glow, gw, gh, pad)) = bank.glow_named(key, GlowPreset::TrophyYellow)
            {
                let sx = sz / bw.max(1) as f32;
                let sy = sz / bh.max(1) as f32;
                canvas.draw_tex_rect_blur(
                    glow,
                    Rect::new(
                        rect.x - pad as f32 * sx,
                        rect.y - pad as f32 * sy,
                        gw as f32 * sx,
                        gh as f32 * sy,
                    ),
                    None,
                    1.0,
                    ta,
                    t_blur,
                );
            }
            canvas.draw_tex_rect_blur(tid, rect, None, 1.0, ta, t_blur);
        }
        if summary.age >= 3.3 {
            let lu = ((summary.age - 3.3) / 1.2).clamp(0.0, 1.0);
            let la = back_out(lu).clamp(0.0, 1.0) * fade_late;
            fonts.draw_fx(
                canvas,
                FontFamily::Pricedown,
                650.0,
                415.0,
                24.0,
                1.0,
                1.0,
                1.0,
                1.0,
                (255, 255, 255),
                Some((0, 0, 0)),
                TextAlign::Center,
                summary.result.trophy.badge_label(),
                1.0,
                la,
                [1.0; 3],
                ((1.0 - lu) * 8.0, 0.0),
            );
        }
    }

    if summary.age >= 2.9 {
        let ba = strong_in(((summary.age - 2.9) / 0.8).clamp(0.0, 1.0)) * fade_late;
        let r_cont = Rect::new(255.0, 494.0, 320.0, 38.0);
        let pulse = 0.5 + 0.5 * (summary.age * std::f32::consts::PI / 0.9).sin();
        let glow_blur = 4.0 + 12.0 * pulse;
        fonts.draw_glow_alpha(
            canvas,
            FontFamily::Neuropol,
            CENTER_X,
            498.0,
            36.0,
            1.0,
            TextAlign::Center,
            "play next level",
            (0x40, 0xD1, 0xAF),
            Some((0x05, 0x0D, 0x2B)),
            (255, 255, 255),
            glow_blur,
            0.85 * ba,
        );
        let allow_hov = summary.hiding.is_none();
        draw_text_button(
            canvas,
            fonts,
            CENTER_X,
            498.0,
            36.0,
            TextAlign::Center,
            "play next level",
            allow_hov && r_cont.contains(mouse_pos.0, mouse_pos.1),
            Some((0x40, 0xD1, 0xAF)),
            ba,
        );
        draw_text_button(
            canvas,
            fonts,
            CENTER_X,
            537.0,
            26.0,
            TextAlign::Center,
            "retry level",
            allow_hov && Rect::new(305.0, 535.0, 220.0, 28.0).contains(mouse_pos.0, mouse_pos.1),
            Some((0x40, 0xD1, 0xAF)),
            ba,
        );
        draw_text_button(
            canvas,
            fonts,
            CENTER_X,
            564.0,
            26.0,
            TextAlign::Center,
            "main menu",
            allow_hov && Rect::new(305.0, 564.0, 220.0, 28.0).contains(mouse_pos.0, mouse_pos.1),
            Some((0x40, 0xD1, 0xAF)),
            ba,
        );
    }
}
