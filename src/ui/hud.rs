// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::{Difficulty, LevelSim, MatchSim};
use crate::render::easing::{back_in, strong_out};
use crate::render::{
    BoardLayout, Canvas, FontFamily, FontSet, GlowPreset, LOGICAL_W, Rect, TexBank, TextAlign,
    draw_tutorial_bubble,
};
use crate::ui::widgets::{UiAction, UiButton};

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
) -> Vec<UiButton> {
    let mut out = Vec::new();
    if hiding_board {
        return out;
    }
    if let Some(cs) = sim {
        if cs.active_tutorial().is_some() {
            out.push(UiButton {
                id: "btn_skip_tut",
                rect: Rect::new(620.0, 10.0, 160.0, 70.0),
                action: UiAction::SkipTutorials,
                enabled: true,
                hover_sfx: None,
            });
            out.push(UiButton {
                id: "btn_tut_next",
                rect: Rect::new(0.0, 0.0, LOGICAL_W as f32, 600.0),
                action: UiAction::NextTutorial,
                enabled: false,
                hover_sfx: None,
            });
        } else if cs.is_started && !cs.finished {
            out.push(UiButton {
                id: "btn_pause",
                rect: Rect::new(10.0, 545.0, 50.0, 50.0),
                action: UiAction::PauseGame,
                enabled: true,
                hover_sfx: None,
            });
            if !cs.is_accelerated {
                out.push(UiButton {
                    id: "btn_ff",
                    rect: Rect::new(70.0, 545.0, 50.0, 50.0),
                    action: UiAction::FastForward,
                    enabled: true,
                    hover_sfx: None,
                });
            }
        }
    } else if let Some(ms) = match_sim
        && ms.is_started
        && !ms.finished
    {
        out.push(UiButton {
            id: "btn_pause",
            rect: Rect::new(10.0, 545.0, 50.0, 50.0),
            action: UiAction::PauseGame,
            enabled: true,
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
    tut_age: f32,
    mouse_pos: (f32, f32),
    hiding_board: bool,
) {
    if !hiding_board && sim.is_started && !sim.finished && sim.active_tutorial().is_none() {
        draw_icon(
            canvas,
            bank,
            Rect::new(10.0, 545.0, 50.0, 50.0),
            "PauseButtonBitmap",
            mouse_pos,
        );
        if !sim.is_accelerated {
            draw_icon(
                canvas,
                bank,
                Rect::new(70.0, 545.0, 50.0, 50.0),
                "FastForwardButtonBitmap",
                mouse_pos,
            );
        }
    }
    if sim.active_tutorial().is_some() {
        draw_icon(
            canvas,
            bank,
            Rect::new(620.0, 10.0, 160.0, 70.0),
            "SkipTutorialButtonBitmap",
            mouse_pos,
        );
    }

    draw_bottom_counters(
        canvas,
        fonts,
        true,
        total_score + sim.score,
        sim.moves,
        sim.elapsed_ms as u32,
        sim.level.par_moves,
        sim.level.par_time_ms,
        sim.difficulty,
    );

    if let Some(msg) = sim.active_tutorial() {
        if let Some(&(tx, ty)) = msg.tiles.first() {
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
    } else if sim.is_started && !sim.is_accelerated && sim.path_complete && sim.level.index <= 4 {
        let hint = if sim.difficulty == Difficulty::Easy {
            "Now click here to finish the puzzle!"
        } else {
            "Done already?\nFASTFORWARD for bonus points!"
        };
        draw_tutorial_bubble(canvas, fonts, 95.0, 560.0, hint, false, 1.0);
    }
}

pub fn render_match_hud(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    sim: &MatchSim,
    total_score: u32,
    mouse_pos: (f32, f32),
    hiding_board: bool,
) {
    if !hiding_board && sim.is_started && !sim.finished {
        draw_icon(
            canvas,
            bank,
            Rect::new(10.0, 545.0, 50.0, 50.0),
            "PauseButtonBitmap",
            mouse_pos,
        );
    }
    let header = if sim.mirror_mode {
        "MIRROR THIS:"
    } else {
        "MATCH THIS:"
    };
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        39.0,
        57.0,
        24.0,
        (0xEA, 0xEA, 0xEA),
        (5, 13, 43),
        TextAlign::Left,
        header,
    );

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
        Some(stroke),
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
        Some(stroke),
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
        stroke,
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
        Some(stroke),
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
        stroke,
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
    let col = (0xC9, 0xDA, 0x41);
    let st = Some((0x05, 0x0D, 0x2B));
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
        Some((0x00, 0x00, 0x40)),
        TextAlign::Left,
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
    }
    canvas.draw_tex_rect(tid, rect, None, 1.0, 1.0);
}
