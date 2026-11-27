// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::{Difficulty, LevelSim, MatchSim};
use crate::render::easing::{back_in, quad_out, strong_out};
use crate::render::{
    BoardLayout, Canvas, FontFamily, FontSet, GlowPreset, LOGICAL_W, Rect, TexBank, TextAlign,
    TextOutline, draw_tutorial_bubble,
};
use crate::ui::widgets::{UiAction, UiButton};

#[derive(Debug, Clone, Default)]
pub struct HudAnimState {
    pub overlay_alpha: f32,
    overlay_start: f32,
    overlay_target: f32,
    overlay_elapsed: f32,
    overlay_dur: f32,
    pub action_visible: bool,
    pub action_alpha: f32,
    action_start: f32,
    action_target: f32,
    action_elapsed: f32,
    action_dur: f32,
    pub skip_visible: bool,
    pub skip_alpha: f32,
    skip_alpha_start: f32,
    skip_alpha_target: f32,
    skip_alpha_elapsed: f32,
    skip_alpha_dur: f32,
    pub skip_slide_elapsed: f32,
}

fn step_tween(
    val: &mut f32,
    start: f32,
    target: f32,
    elapsed: &mut f32,
    dur: &mut f32,
    dt: f32,
    ease: fn(f32) -> f32,
) -> bool {
    if *dur <= 0.0 {
        *val = target;
        return false;
    }
    *elapsed = (*elapsed + dt).min(*dur);
    let t = (*elapsed / *dur).clamp(0.0, 1.0);
    *val = start + (target - start) * ease(t);
    if t >= 1.0 {
        *val = target;
        *dur = 0.0;
        true
    } else {
        false
    }
}

impl HudAnimState {
    #[cfg(test)]
    pub fn fully_visible() -> Self {
        Self {
            overlay_alpha: 1.0,
            overlay_start: 1.0,
            overlay_target: 1.0,
            overlay_elapsed: 0.0,
            overlay_dur: 0.0,
            action_visible: true,
            action_alpha: 1.0,
            action_start: 1.0,
            action_target: 1.0,
            action_elapsed: 0.0,
            action_dur: 0.0,
            skip_visible: false,
            skip_alpha: 0.0,
            skip_alpha_start: 0.0,
            skip_alpha_target: 0.0,
            skip_alpha_elapsed: 0.0,
            skip_alpha_dur: 0.0,
            skip_slide_elapsed: 0.8,
        }
    }

    pub fn show_overlay(&mut self) {
        if (self.overlay_alpha - 1.0).abs() < 1e-4 && self.overlay_target >= 1.0 {
            return;
        }
        self.action_visible = false;
        self.action_alpha = 0.0;
        self.action_start = 0.0;
        self.action_target = 0.0;
        self.action_elapsed = 0.0;
        self.action_dur = 0.0;
        self.overlay_alpha = 0.0;
        self.overlay_start = 0.0;
        self.overlay_target = 1.0;
        self.overlay_elapsed = 0.0;
        self.overlay_dur = 1.0;
    }

    pub fn hide_overlay(&mut self) {
        if self.overlay_alpha <= 0.004 && self.overlay_target <= 0.0 {
            return;
        }
        self.overlay_start = self.overlay_alpha;
        self.overlay_target = 0.0;
        self.overlay_elapsed = 0.0;
        self.overlay_dur = 1.0;
    }

    pub fn show_action_buttons(&mut self) {
        self.action_visible = true;
        if (self.action_alpha - 1.0).abs() < 1e-4 && self.action_target >= 1.0 {
            return;
        }
        self.action_start = self.action_alpha;
        self.action_target = 1.0;
        self.action_elapsed = 0.0;
        self.action_dur = 0.8;
    }

    pub fn hide_action_buttons(&mut self) {
        if !self.action_visible || (self.action_alpha <= 0.004 && self.action_target <= 0.0) {
            return;
        }
        self.action_start = self.action_alpha;
        self.action_target = 0.0;
        self.action_elapsed = 0.0;
        self.action_dur = 0.8;
    }

    pub fn show_skip_tutorial(&mut self) {
        self.skip_visible = true;
        self.skip_alpha = 1.0;
        self.skip_alpha_start = 1.0;
        self.skip_alpha_target = 1.0;
        self.skip_alpha_elapsed = 0.0;
        self.skip_alpha_dur = 0.0;
        self.skip_slide_elapsed = 0.0;
    }

    pub fn hide_skip_tutorial(&mut self) {
        if !self.skip_visible || (self.skip_alpha <= 0.004 && self.skip_alpha_target <= 0.0) {
            return;
        }
        self.skip_alpha_start = self.skip_alpha;
        self.skip_alpha_target = 0.0;
        self.skip_alpha_elapsed = 0.0;
        self.skip_alpha_dur = 0.8;
    }

    pub fn skip_tutorial_y(&self) -> f32 {
        -70.0 + 80.0 * strong_out((self.skip_slide_elapsed / 0.8).clamp(0.0, 1.0))
    }

    pub fn actions_clickable(&self) -> bool {
        self.overlay_alpha >= 0.999 && self.action_visible && self.action_alpha >= 0.999
    }

    pub fn skip_clickable(&self) -> bool {
        self.overlay_alpha >= 0.999 && self.skip_visible && self.skip_alpha >= 0.999
    }

    pub fn update(&mut self, dt: f32) {
        step_tween(
            &mut self.overlay_alpha,
            self.overlay_start,
            self.overlay_target,
            &mut self.overlay_elapsed,
            &mut self.overlay_dur,
            dt,
            quad_out,
        );
        if step_tween(
            &mut self.action_alpha,
            self.action_start,
            self.action_target,
            &mut self.action_elapsed,
            &mut self.action_dur,
            dt,
            strong_out,
        ) && self.action_target <= 0.0
        {
            self.action_visible = false;
        }
        if self.skip_visible {
            self.skip_slide_elapsed = (self.skip_slide_elapsed + dt).min(0.8);
            if step_tween(
                &mut self.skip_alpha,
                self.skip_alpha_start,
                self.skip_alpha_target,
                &mut self.skip_alpha_elapsed,
                &mut self.skip_alpha_dur,
                dt,
                strong_out,
            ) && self.skip_alpha_target <= 0.0
            {
                self.skip_visible = false;
            }
        }
    }
}

pub fn campaign_board_area() -> Rect {
    Rect::new(50.0, 50.0, 725.0, 525.0)
}

pub fn match_play_area() -> Rect {
    Rect::new(260.0, 30.0, 510.0, 525.0)
}

pub fn match_preview_area() -> Rect {
    Rect::new(30.0, 92.0, 200.0, 200.0)
}

pub fn build_playing_buttons(
    sim: Option<&LevelSim>,
    match_sim: Option<&MatchSim>,
    hiding_board: bool,
    hud_anim: &HudAnimState,
) -> Vec<UiButton> {
    let mut out = Vec::new();
    if hiding_board {
        return out;
    }
    if let Some(cs) = sim {
        if hud_anim.skip_visible && hud_anim.skip_alpha > 0.004 {
            out.push(UiButton {
                id: "btn_skip_tut",
                rect: Rect::new(620.0, hud_anim.skip_tutorial_y(), 160.0, 70.0),
                action: UiAction::SkipTutorials,
                enabled: hud_anim.skip_clickable(),
                hover_sfx: None,
            });
        }
        if cs.active_tutorial().is_some() {
            out.push(UiButton {
                id: "btn_tut_next",
                rect: Rect::new(0.0, 0.0, LOGICAL_W as f32, 600.0),
                action: UiAction::NextTutorial,
                enabled: false,
                hover_sfx: None,
            });
        } else if cs.is_started
            && !cs.finished
            && hud_anim.action_visible
            && hud_anim.action_alpha > 0.004
        {
            let clickable = hud_anim.actions_clickable();
            out.push(UiButton {
                id: "btn_pause",
                rect: Rect::new(10.0, 545.0, 50.0, 50.0),
                action: UiAction::PauseGame,
                enabled: clickable,
                hover_sfx: None,
            });
            if !cs.is_accelerated {
                out.push(UiButton {
                    id: "btn_ff",
                    rect: Rect::new(70.0, 545.0, 50.0, 50.0),
                    action: UiAction::FastForward,
                    enabled: clickable,
                    hover_sfx: None,
                });
            }
        }
    } else if let Some(ms) = match_sim
        && ms.is_started
        && !ms.finished
        && hud_anim.action_visible
        && hud_anim.action_alpha > 0.004
    {
        out.push(UiButton {
            id: "btn_pause",
            rect: Rect::new(10.0, 545.0, 50.0, 50.0),
            action: UiAction::PauseGame,
            enabled: hud_anim.actions_clickable(),
            hover_sfx: None,
        });
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub fn render_campaign_hud(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    sim: &LevelSim,
    layout: BoardLayout,
    total_score: u32,
    display_moves: u32,
    display_time_ms: u32,
    tut_age: f32,
    ff_hint_age: Option<f32>,
    mouse_pos: (f32, f32),
    hiding_board: bool,
    allow_ff_hint: bool,
    hud_anim: &HudAnimState,
    show_hints: bool,
) {
    if hud_anim.overlay_alpha > 0.004 {
        let hud_start = canvas.cmds.len();
        if !hiding_board && hud_anim.action_visible && hud_anim.action_alpha > 0.004 {
            let act_start = canvas.cmds.len();
            let hov_pos = if hud_anim.actions_clickable() {
                mouse_pos
            } else {
                (-999.0, -999.0)
            };
            draw_icon(
                canvas,
                bank,
                Rect::new(10.0, 545.0, 50.0, 50.0),
                "PauseButtonBitmap",
                hov_pos,
            );
            draw_icon(
                canvas,
                bank,
                Rect::new(70.0, 545.0, 50.0, 50.0),
                "FastForwardButtonBitmap",
                hov_pos,
            );
            canvas.transform_cmds_since(act_start, 65.0, 570.0, 1.0, hud_anim.action_alpha);
        }
        if hud_anim.skip_visible && hud_anim.skip_alpha > 0.004 {
            let skip_start = canvas.cmds.len();
            let hov_pos = if hud_anim.skip_clickable() {
                mouse_pos
            } else {
                (-999.0, -999.0)
            };
            draw_icon(
                canvas,
                bank,
                Rect::new(620.0, hud_anim.skip_tutorial_y(), 160.0, 70.0),
                "SkipTutorialButtonBitmap",
                hov_pos,
            );
            canvas.transform_cmds_since(
                skip_start,
                700.0,
                hud_anim.skip_tutorial_y() + 35.0,
                1.0,
                hud_anim.skip_alpha,
            );
        }

        draw_bottom_counters(
            canvas,
            fonts,
            true,
            total_score,
            display_moves,
            display_time_ms,
            sim.level.par_moves,
            sim.level.par_time_ms,
            sim.difficulty,
        );

        if show_hints
            && sim.active_tutorial().is_none()
            && allow_ff_hint
            && sim.is_started
            && !sim.finished
            && !sim.is_accelerated
            && sim.path_complete
            && sim.level.index <= 4
            && let Some(ff_age) = ff_hint_age
        {
            let hint = if sim.difficulty == Difficulty::Easy {
                "Now click here to finish the puzzle!"
            } else {
                "Done already?\nFASTFORWARD for bonus points!"
            };
            draw_tutorial_bubble(canvas, fonts, 95.0, 560.0, hint, false, ff_age);
        }
        canvas.transform_cmds_since(hud_start, 400.0, 300.0, 1.0, hud_anim.overlay_alpha);
    }

    if show_hints && let Some(msg) = sim.active_tutorial() {
        for &(tx, ty) in &msg.tiles {
            let (tcx, tcy) = layout.tile_center(tx, ty);
            draw_tutorial_bubble(
                canvas,
                fonts,
                tcx,
                tcy,
                &msg.text,
                msg.click_to_continue(),
                tut_age,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_match_hud(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    sim: &MatchSim,
    total_score: u32,
    mouse_pos: (f32, f32),
    hiding_board: bool,
    preview_alpha: f32,
    hud_anim: &HudAnimState,
    show_preview_header: bool,
) {
    let header = if sim.mirror_mode {
        "MIRROR THIS:"
    } else {
        "MATCH THIS:"
    };
    if show_preview_header && preview_alpha > 0.004 {
        fonts.draw_styled_alpha(
            canvas,
            FontFamily::Neuropol,
            39.0,
            57.0,
            24.0,
            1.0,
            1.0,
            (0xEA, 0xEA, 0xEA),
            Some(TextOutline::stroke((0, 0, 64))),
            TextAlign::Left,
            header,
            preview_alpha,
        );
    }

    if hud_anim.overlay_alpha > 0.004 {
        let hud_start = canvas.cmds.len();
        if !hiding_board
            && sim.is_started
            && !sim.finished
            && hud_anim.action_visible
            && hud_anim.action_alpha > 0.004
        {
            let act_start = canvas.cmds.len();
            let hov_pos = if hud_anim.actions_clickable() {
                mouse_pos
            } else {
                (-999.0, -999.0)
            };
            draw_icon(
                canvas,
                bank,
                Rect::new(10.0, 545.0, 50.0, 50.0),
                "PauseButtonBitmap",
                hov_pos,
            );
            canvas.transform_cmds_since(act_start, 35.0, 570.0, 1.0, hud_anim.action_alpha);
        }

        draw_bottom_counters(
            canvas,
            fonts,
            false,
            total_score,
            0,
            0,
            0,
            0,
            Difficulty::Medium,
        );
        canvas.transform_cmds_since(hud_start, 400.0, 300.0, 1.0, hud_anim.overlay_alpha);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_bottom_counters(
    canvas: &mut Canvas,
    fonts: &FontSet,
    show_moves_time: bool,
    total_score: u32,
    moves: u32,
    elapsed_ms: u32,
    par_moves_raw: u32,
    par_time_raw_ms: u32,
    difficulty: Difficulty,
) {
    let stroke = (0x00, 0x00, 0x40);
    let par_stroke = (0x16, 0x16, 0x16);
    let label_y = 568.0;
    let val_y = 571.0;

    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        690.0,
        label_y,
        20.0,
        (255, 255, 255),
        stroke,
        TextAlign::Right,
        "Score:",
    );
    fonts.draw_digits_fx(
        canvas,
        FontFamily::Neuropol,
        778.0,
        val_y,
        16.0,
        1.0,
        1.0,
        1.0,
        (255, 255, 255),
        Some(TextOutline::shadowed(stroke)),
        TextAlign::Right,
        &total_score.to_string(),
        1.0,
        (0.0, 0.0),
    );

    if !show_moves_time {
        return;
    }

    let (pm, pt_ms, par_col) = if difficulty == Difficulty::Hard {
        (par_moves_raw, par_time_raw_ms, (0xC0, 0xC0, 0xC0))
    } else {
        (
            ((par_moves_raw as f32) * 1.25).floor() as u32,
            ((par_time_raw_ms as f32) * 1.25).floor() as u32,
            (0xFF, 0xFF, 0x33),
        )
    };

    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        488.0,
        label_y,
        20.0,
        (255, 255, 255),
        stroke,
        TextAlign::Right,
        "Moves:",
    );
    fonts.draw_digits_fx(
        canvas,
        FontFamily::Neuropol,
        536.0,
        val_y,
        16.0,
        1.0,
        1.0,
        1.0,
        (255, 255, 255),
        Some(TextOutline::shadowed(stroke)),
        TextAlign::Right,
        &moves.to_string(),
        1.0,
        (0.0, 0.0),
    );
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        543.0,
        val_y,
        16.0,
        par_col,
        par_stroke,
        TextAlign::Left,
        &format!("/ {}", pm),
    );

    let secs = elapsed_ms / 1000;
    let psecs = pt_ms / 1000;
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        270.0,
        label_y,
        20.0,
        (255, 255, 255),
        stroke,
        TextAlign::Right,
        "Time:",
    );
    fonts.draw_digits_fx(
        canvas,
        FontFamily::Neuropol,
        326.0,
        val_y,
        16.0,
        1.0,
        1.0,
        1.0,
        (255, 255, 255),
        Some(TextOutline::shadowed(stroke)),
        TextAlign::Right,
        &format!("{}:{:02}", secs / 60, secs % 60),
        1.0,
        (0.0, 0.0),
    );
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        333.0,
        val_y,
        16.0,
        par_col,
        par_stroke,
        TextAlign::Left,
        &format!("/ {}:{:02}", psecs / 60, psecs % 60),
    );
}

pub fn draw_level_info_screen(
    canvas: &mut Canvas,
    fonts: &FontSet,
    title: &str,
    description: &str,
    elapsed: f32,
    delay: f32,
) {
    let t = elapsed - delay;
    if !(0.0..=4.8).contains(&t) {
        return;
    }
    let exit_start = 4.0;
    let (tx, ty, dx, dy, blur) = if t < 1.0 {
        let e = strong_out(t);
        (
            400.0,
            -100.0 + 300.0 * e,
            400.0,
            620.0 - 370.0 * e,
            (0.0, (1.0 - t) * 8.0),
        )
    } else if t < exit_start {
        (400.0, 200.0, 400.0, 250.0, (0.0, 0.0))
    } else {
        let u = ((t - exit_start) / 0.8).clamp(0.0, 1.0);
        let e = back_in(u);
        let tw = fonts.measure_width(FontFamily::Neuropol, title, 48.0);
        let dw = fonts.measure_width(FontFamily::Neuropol, description, 24.0);
        (
            400.0 + (-tw * 0.5 - 32.0 - 400.0) * e,
            200.0,
            400.0 + (800.0 + dw * 0.5 + 32.0 - 400.0) * e,
            250.0,
            (u * 8.0, 0.0),
        )
    };
    let col = (0xC9, 0xD8, 0xE1);
    let st = Some(TextOutline::shadowed((0x05, 0x0D, 0x2B)));
    fonts.draw_fx(
        canvas,
        FontFamily::Neuropol,
        tx,
        ty,
        48.0,
        1.0,
        1.0,
        1.0,
        1.0,
        col,
        st,
        TextAlign::Center,
        title,
        1.0,
        1.0,
        [1.0; 3],
        blur,
    );
    fonts.draw_fx(
        canvas,
        FontFamily::Neuropol,
        dx,
        dy,
        24.0,
        1.0,
        1.0,
        1.0,
        1.0,
        col,
        st,
        TextAlign::Center,
        description,
        1.0,
        1.0,
        [1.0; 3],
        blur,
    );
}

pub fn draw_status_toast(canvas: &mut Canvas, fonts: &FontSet, text: &str, age: f32) {
    if text.is_empty() || age > 4.5 {
        return;
    }
    let alpha = if age < 0.8 {
        (age / 0.8).clamp(0.0, 1.0)
    } else if age < 3.5 {
        1.0
    } else {
        (1.0 - (age - 3.5)).clamp(0.0, 1.0)
    };
    fonts.draw_styled_alpha(
        canvas,
        FontFamily::Neuropol,
        4.0,
        4.0,
        20.0,
        1.0,
        1.0,
        (255, 255, 255),
        Some(TextOutline::shadowed((0x00, 0x00, 0x40))),
        TextAlign::Left,
        text,
        alpha,
    );
}

pub fn draw_editor_toast(canvas: &mut Canvas, fonts: &FontSet, text: &str, age: f32) {
    if text.is_empty() || age > 4.5 {
        return;
    }
    let alpha = if age < 0.4 {
        (age / 0.4).clamp(0.0, 1.0)
    } else if age < 3.5 {
        1.0
    } else {
        (1.0 - (age - 3.5)).clamp(0.0, 1.0)
    };
    let w = (fonts.measure_width(FontFamily::Neuropol, text, 14.0) + 28.0).clamp(160.0, 500.0);
    let r = Rect::new(524.0 - w * 0.5, 494.0, w, 28.0);
    let bg_a = (220.0 * alpha).round() as u8;
    let bd_a = (240.0 * alpha).round() as u8;
    canvas.draw_rounded_rect(
        r,
        8.0,
        (0x05, 0x0D, 0x2B, bg_a),
        Some((0x59, 0x95, 0xC1, bd_a, 2.0)),
    );
    fonts.draw_styled_alpha(
        canvas,
        FontFamily::Neuropol,
        524.0,
        500.0,
        14.0,
        1.0,
        1.0,
        (255, 255, 255),
        Some(TextOutline::stroke((0x05, 0x0D, 0x2B))),
        TextAlign::Center,
        text,
        alpha,
    );
}

fn draw_icon(
    canvas: &mut Canvas,
    bank: &TexBank,
    rect: Rect,
    bmp_key: &str,
    mouse_pos: (f32, f32),
) {
    let hovered = rect.contains(mouse_pos.0, mouse_pos.1);
    let Some((tid, bw, bh)) = bank.named(bmp_key) else {
        return;
    };
    if hovered && let Some((gid, gw, gh, pad)) = bank.glow_named(bmp_key, GlowPreset::ButtonWhite) {
        let sx = rect.w / bw.max(1) as f32;
        let sy = rect.h / bh.max(1) as f32;
        canvas.draw_tex_rect(
            gid,
            Rect::new(
                rect.x - pad as f32 * sx,
                rect.y - pad as f32 * sy,
                gw as f32 * sx,
                gh as f32 * sy,
            ),
            None,
            1.0,
            1.0,
        );
        canvas.boost_last_sprite(1.25);
    }
    canvas.draw_tex_rect(tid, rect, None, 1.0, 1.0);
}
