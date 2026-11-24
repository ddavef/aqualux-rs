// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::AssetBundle;
use crate::game::generator::{GenDifficulty, MIN_GEN_COLS, MIN_GEN_ROWS};
use crate::game::level_io::{MAX_GRID_COLS, MAX_GRID_ROWS};
use crate::game::{AwardKind, Difficulty};
use crate::render::{
    Canvas, FontFamily, FontSet, GlowPreset, LOGICAL_H, LOGICAL_W, Rect, TexBank, TextAlign,
};
use crate::save::SaveData;
use crate::ui::EndlessConfig;
use crate::ui::editor::{build_level_props_buttons, render_level_props_modal};
use crate::ui::profiles::{
    build_enter_name_buttons, build_profile_modal_buttons, render_enter_name_modal,
    render_profile_modal,
};
use crate::ui::trophy_room::{build_trophy_room_buttons, render_trophy_room};
use crate::ui::widgets::{
    ModalState, UiAction, UiButton, draw_checkbox, draw_modal_bubble, draw_modal_button,
    draw_radio, draw_slider, draw_text_button,
};

pub const OPTIONS_SLIDER_X: f32 = 435.0;
pub const OPTIONS_FX_Y: f32 = 225.0;
pub const OPTIONS_MUSIC_Y: f32 = 255.0;

pub fn build_pause_buttons() -> Vec<UiButton> {
    let bx = 270.0;
    vec![
        UiButton {
            id: "p_resume",
            rect: Rect::new(bx, 260.0, 260.0, 34.0),
            action: UiAction::ResumeGame,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "p_opts",
            rect: Rect::new(bx, 300.0, 260.0, 34.0),
            action: UiAction::OpenOptions,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "p_menu",
            rect: Rect::new(bx, 340.0, 260.0, 34.0),
            action: UiAction::MainMenuPrompt,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
    ]
}

pub fn render_pause_menu(
    canvas: &mut Canvas,
    fonts: &FontSet,
    exit_label: &str,
    mouse_pos: (f32, f32),
) {
    canvas.fill_rect_alpha(
        Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
        0,
        0,
        0,
        95,
    );
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        400.0,
        180.0,
        45.0,
        (0xF4, 0xF5, 0xF7),
        (5, 13, 43),
        TextAlign::Center,
        "game paused",
    );
    for (label, y) in [
        ("Resume Game", 260.0),
        ("Options", 300.0),
        (exit_label, 340.0),
    ] {
        let r = Rect::new(270.0, y, 260.0, 34.0);
        draw_text_button(
            canvas,
            fonts,
            400.0,
            y,
            31.0,
            TextAlign::Center,
            label,
            r.contains(mouse_pos.0, mouse_pos.1),
            None,
            1.0,
        );
    }
}

pub fn build_modal_buttons(
    modal: &ModalState,
    assets: &AssetBundle,
    bank: &TexBank,
    fonts: &FontSet,
    save: &SaveData,
) -> Vec<UiButton> {
    match modal {
        ModalState::TrophyRoom { page, .. } => build_trophy_room_buttons(assets, bank, save, *page),
        ModalState::Options { allow_difficulty } => {
            let bx = 215.0;
            let base_y = 160.0;
            let diff_y = base_y + 125.0;
            let cb_y = if *allow_difficulty {
                base_y + 168.0
            } else {
                base_y + 138.0
            };
            let win_y = cb_y + 30.0;
            let fps_y = win_y + 30.0;
            let done_y = fps_y + 38.0;
            let mut out = Vec::new();
            if *allow_difficulty {
                out.push(UiButton {
                    id: "opt_diff",
                    rect: Rect::new(OPTIONS_SLIDER_X - 55.0, diff_y, 175.0, 24.0),
                    action: UiAction::OpenOptionsDifficulty,
                    enabled: true,
                    hover_sfx: Some("PopTwoSound"),
                });
            }
            out.push(UiButton {
                id: "opt_cb",
                rect: Rect::new(bx + 20.0, cb_y, 260.0, 22.0),
                action: UiAction::ToggleColorBlind,
                enabled: true,
                hover_sfx: None,
            });
            out.push(UiButton {
                id: "opt_win",
                rect: Rect::new(bx + 20.0, win_y, 260.0, 22.0),
                action: UiAction::ToggleWindowed,
                enabled: true,
                hover_sfx: None,
            });
            out.push(UiButton {
                id: "opt_fps",
                rect: Rect::new(OPTIONS_SLIDER_X - 20.0, fps_y - 3.0, 145.0, 24.0),
                action: UiAction::CycleFps,
                enabled: true,
                hover_sfx: Some("PopTwoSound"),
            });
            out.push(UiButton {
                id: "opt_done",
                rect: Rect::new(400.0 - 47.0, done_y, 94.0, 53.0),
                action: UiAction::CloseTopModal,
                enabled: true,
                hover_sfx: None,
            });
            out
        }
        ModalState::Difficulty { ok_only, .. } => {
            let mut out = vec![
                UiButton {
                    id: "d_easy",
                    rect: Rect::new(265.0, 290.0, 260.0, 26.0),
                    action: UiAction::SelectDifficultyRadio(Difficulty::Easy),
                    enabled: true,
                    hover_sfx: None,
                },
                UiButton {
                    id: "d_med",
                    rect: Rect::new(265.0, 320.0, 260.0, 26.0),
                    action: UiAction::SelectDifficultyRadio(Difficulty::Medium),
                    enabled: true,
                    hover_sfx: None,
                },
                UiButton {
                    id: "d_hard",
                    rect: Rect::new(265.0, 350.0, 260.0, 26.0),
                    action: UiAction::SelectDifficultyRadio(Difficulty::Hard),
                    enabled: true,
                    hover_sfx: None,
                },
            ];
            if *ok_only {
                out.push(UiButton {
                    id: "d_ok",
                    rect: Rect::new(400.0 - 47.0, 396.0, 94.0, 53.0),
                    action: UiAction::ConfirmDifficultyModal,
                    enabled: true,
                    hover_sfx: None,
                });
            } else {
                out.push(UiButton {
                    id: "d_play",
                    rect: Rect::new(298.0, 396.0, 94.0, 53.0),
                    action: UiAction::ConfirmDifficultyModal,
                    enabled: true,
                    hover_sfx: None,
                });
                out.push(UiButton {
                    id: "d_back",
                    rect: Rect::new(408.0, 396.0, 94.0, 53.0),
                    action: UiAction::CloseTopModal,
                    enabled: true,
                    hover_sfx: None,
                });
            }
            out
        }
        ModalState::InGameDifficulty { .. } => vec![
            UiButton {
                id: "igd_chk",
                rect: Rect::new(280.0, 325.0, 240.0, 24.0),
                action: UiAction::ToggleLevelOnlyCheckbox,
                enabled: true,
                hover_sfx: None,
            },
            UiButton {
                id: "igd_yes",
                rect: Rect::new(298.0, 368.0, 94.0, 53.0),
                action: UiAction::InGameDiffYes,
                enabled: true,
                hover_sfx: None,
            },
            UiButton {
                id: "igd_no",
                rect: Rect::new(408.0, 368.0, 94.0, 53.0),
                action: UiAction::InGameDiffNo,
                enabled: true,
                hover_sfx: None,
            },
        ],
        ModalState::Profile { items, .. } => build_profile_modal_buttons(items.len()),
        ModalState::EnterName {
            prompt,
            allow_cancel,
            ..
        } => build_enter_name_buttons(fonts, prompt, *allow_cancel),
        ModalState::TextPrompt { prompt, .. } => build_enter_name_buttons(fonts, prompt, true)
            .into_iter()
            .map(|mut b| {
                if b.action == UiAction::SubmitName {
                    b.action = UiAction::SubmitTextPrompt;
                }
                b
            })
            .collect(),
        ModalState::Question {
            message, no_label, ..
        } => {
            let lines = fonts.wrap_lines(FontFamily::Neuropol, message, 22.0, 370.0);
            let msg_h = (lines.len().max(1) as f32) * 26.0 + 15.0;
            let total_h = 15.0 + msg_h + 30.0 + 53.0 + 15.0;
            let by = (600.0 - total_h) * 0.5 + 15.0 + msg_h + 30.0;
            if no_label.is_some() {
                vec![
                    UiButton {
                        id: "q_yes",
                        rect: Rect::new(400.0 - 99.0, by, 94.0, 53.0),
                        action: UiAction::QuestionYes,
                        enabled: true,
                        hover_sfx: None,
                    },
                    UiButton {
                        id: "q_no",
                        rect: Rect::new(405.0, by, 94.0, 53.0),
                        action: UiAction::QuestionNo,
                        enabled: true,
                        hover_sfx: None,
                    },
                ]
            } else {
                vec![UiButton {
                    id: "q_ok",
                    rect: Rect::new(400.0 - 47.0, by, 94.0, 53.0),
                    action: UiAction::QuestionYes,
                    enabled: true,
                    hover_sfx: None,
                }]
            }
        }
        ModalState::Award { .. } => vec![UiButton {
            id: "aw_ok",
            rect: Rect::new(400.0 - 47.0, 378.0, 94.0, 53.0),
            action: UiAction::DismissAward,
            enabled: true,
            hover_sfx: None,
        }],
        ModalState::LevelProps(props) => build_level_props_buttons(props),
        ModalState::EndlessSetup(cfg) => vec![
            UiButton {
                id: "end_easy",
                rect: Rect::new(225.0, 198.0, 355.0, 26.0),
                action: UiAction::EndlessToggleDifficulty(GenDifficulty::Easy),
                enabled: true,
                hover_sfx: None,
            },
            UiButton {
                id: "end_med",
                rect: Rect::new(225.0, 232.0, 355.0, 26.0),
                action: UiAction::EndlessToggleDifficulty(GenDifficulty::Medium),
                enabled: true,
                hover_sfx: None,
            },
            UiButton {
                id: "end_hard",
                rect: Rect::new(225.0, 266.0, 355.0, 26.0),
                action: UiAction::EndlessToggleDifficulty(GenDifficulty::Hard),
                enabled: true,
                hover_sfx: None,
            },
            UiButton {
                id: "end_cols_dec",
                rect: Rect::new(392.0, 312.0, 26.0, 24.0),
                action: UiAction::EndlessAdjustMaxCols(-1),
                enabled: cfg.max_cols > MIN_GEN_COLS,
                hover_sfx: Some("PopTwoSound"),
            },
            UiButton {
                id: "end_cols_inc",
                rect: Rect::new(450.0, 312.0, 26.0, 24.0),
                action: UiAction::EndlessAdjustMaxCols(1),
                enabled: cfg.max_cols < MAX_GRID_COLS,
                hover_sfx: Some("PopTwoSound"),
            },
            UiButton {
                id: "end_rows_dec",
                rect: Rect::new(496.0, 312.0, 26.0, 24.0),
                action: UiAction::EndlessAdjustMaxRows(-1),
                enabled: cfg.max_rows > MIN_GEN_ROWS,
                hover_sfx: Some("PopTwoSound"),
            },
            UiButton {
                id: "end_rows_inc",
                rect: Rect::new(554.0, 312.0, 26.0, 24.0),
                action: UiAction::EndlessAdjustMaxRows(1),
                enabled: cfg.max_rows < MAX_GRID_ROWS,
                hover_sfx: Some("PopTwoSound"),
            },
            UiButton {
                id: "end_rules",
                rect: Rect::new(370.0, 348.0, 212.0, 26.0),
                action: UiAction::EndlessCycleRules,
                enabled: true,
                hover_sfx: Some("PopTwoSound"),
            },
            UiButton {
                id: "end_play",
                rect: Rect::new(298.0, 412.0, 94.0, 53.0),
                action: UiAction::StartEndlessRun,
                enabled: cfg.any_enabled(),
                hover_sfx: None,
            },
            UiButton {
                id: "end_back",
                rect: Rect::new(408.0, 412.0, 94.0, 53.0),
                action: UiAction::CloseTopModal,
                enabled: true,
                hover_sfx: None,
            },
        ],
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_modal_stack(
    canvas: &mut Canvas,
    assets: &AssetBundle,
    bank: &TexBank,
    fonts: &FontSet,
    save: &SaveData,
    modals: &[ModalState],
    time: f32,
    mouse_pos: (f32, f32),
) {
    let last_idx = modals.len().saturating_sub(1);
    for (idx, m) in modals.iter().enumerate() {
        let mpos = if idx == last_idx {
            mouse_pos
        } else {
            (-999.0, -999.0)
        };
        match m {
            ModalState::TrophyRoom { page, .. } => {
                render_trophy_room(canvas, assets, bank, fonts, save, *page, mpos);
            }
            ModalState::Options { allow_difficulty } => {
                render_options_modal(canvas, bank, fonts, save, *allow_difficulty, mpos);
            }
            ModalState::Difficulty {
                selected, ok_only, ..
            } => {
                render_difficulty_modal(canvas, bank, fonts, *selected, *ok_only, mpos);
            }
            ModalState::InGameDifficulty { level_only } => {
                render_ingame_diff_modal(canvas, bank, fonts, *level_only, mpos);
            }
            ModalState::Profile {
                items, selected, ..
            } => {
                render_profile_modal(canvas, bank, fonts, items, *selected, mpos);
            }
            ModalState::EnterName {
                prompt,
                input,
                allow_cancel,
                ..
            } => {
                render_enter_name_modal(
                    canvas,
                    bank,
                    fonts,
                    prompt,
                    input,
                    *allow_cancel,
                    time,
                    mpos,
                );
            }
            ModalState::TextPrompt { prompt, input, .. } => {
                render_enter_name_modal(canvas, bank, fonts, prompt, input, true, time, mpos);
            }
            ModalState::Question {
                message,
                yes_label,
                no_label,
                ..
            } => {
                render_question_modal(canvas, bank, fonts, message, yes_label, *no_label, mpos);
            }
            ModalState::Award { kind, .. } => {
                render_award_modal(canvas, bank, fonts, *kind, mpos);
            }
            ModalState::LevelProps(props) => {
                render_level_props_modal(canvas, bank, fonts, props, mpos);
            }
            ModalState::EndlessSetup(cfg) => {
                render_endless_setup_modal(canvas, bank, fonts, *cfg, mpos);
            }
        }
    }
}

fn render_options_modal(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    save: &SaveData,
    allow_difficulty: bool,
    mouse_pos: (f32, f32),
) {
    canvas.fill_rect_alpha(
        Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
        0,
        0,
        0,
        105,
    );
    let bx = 215.0;
    let base_y = 160.0;
    let diff_y = base_y + 125.0;
    let cb_y = if allow_difficulty {
        base_y + 168.0
    } else {
        base_y + 138.0
    };
    let win_y = cb_y + 30.0;
    let fps_y = win_y + 30.0;
    let done_y = fps_y + 38.0;
    let bh = (done_y + 53.0 + 18.0) - base_y;
    draw_modal_bubble(canvas, Rect::new(bx, base_y, 370.0, bh));

    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        400.0,
        base_y + 16.0,
        22.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Center,
        "-Options-",
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        bx + 20.0,
        OPTIONS_FX_Y,
        17.0,
        (255, 255, 255),
        TextAlign::Left,
        "fx volume:",
    );
    draw_slider(
        canvas,
        OPTIONS_SLIDER_X,
        OPTIONS_FX_Y,
        save.config.sfx_volume,
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        bx + 20.0,
        OPTIONS_MUSIC_Y,
        17.0,
        (255, 255, 255),
        TextAlign::Left,
        "music volume:",
    );
    draw_slider(
        canvas,
        OPTIONS_SLIDER_X,
        OPTIONS_MUSIC_Y,
        save.config.music_volume,
    );

    if allow_difficulty {
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            bx + 20.0,
            diff_y,
            17.0,
            (255, 255, 255),
            TextAlign::Left,
            "set difficulty:",
        );
        let dlbl = save
            .active_profile()
            .map(|p| p.difficulty.human_label())
            .unwrap_or("casual (medium)");
        let r_df = Rect::new(OPTIONS_SLIDER_X - 55.0, diff_y, 175.0, 24.0);
        draw_text_button(
            canvas,
            fonts,
            OPTIONS_SLIDER_X + 120.0,
            diff_y + 1.0,
            16.0,
            TextAlign::Right,
            dlbl,
            r_df.contains(mouse_pos.0, mouse_pos.1),
            Some((0x42, 0x83, 0xF3)),
            1.0,
        );
    }

    draw_checkbox(
        canvas,
        fonts,
        bx + 20.0,
        cb_y,
        "colorblind mode",
        save.config.color_blind,
        Rect::new(bx + 20.0, cb_y, 260.0, 22.0).contains(mouse_pos.0, mouse_pos.1),
    );
    draw_checkbox(
        canvas,
        fonts,
        bx + 20.0,
        win_y,
        "windowed mode",
        !save.config.fullscreen,
        Rect::new(bx + 20.0, win_y, 260.0, 22.0).contains(mouse_pos.0, mouse_pos.1),
    );

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        bx + 20.0,
        fps_y - 2.0,
        17.0,
        (255, 255, 255),
        TextAlign::Left,
        "frame rate:",
    );
    let fps_lbl = if save.config.target_fps == 0 {
        "Uncapped".to_string()
    } else {
        format!("{} fps", save.config.target_fps)
    };
    let r_fps = Rect::new(OPTIONS_SLIDER_X - 20.0, fps_y - 3.0, 145.0, 24.0);
    draw_text_button(
        canvas,
        fonts,
        OPTIONS_SLIDER_X + 120.0,
        fps_y - 2.0,
        16.0,
        TextAlign::Right,
        &fps_lbl,
        r_fps.contains(mouse_pos.0, mouse_pos.1),
        Some((0x42, 0x83, 0xF3)),
        1.0,
    );

    let r_done = Rect::new(400.0 - 47.0, done_y, 94.0, 53.0);
    draw_modal_button(
        canvas,
        bank,
        fonts,
        r_done,
        "done",
        r_done.contains(mouse_pos.0, mouse_pos.1),
        true,
    );
}

fn render_difficulty_modal(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    selected: Difficulty,
    ok_only: bool,
    mouse_pos: (f32, f32),
) {
    canvas.fill_rect_alpha(
        Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
        0,
        0,
        0,
        105,
    );
    draw_modal_bubble(canvas, Rect::new(205.0, 180.0, 390.0, 285.0));
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        400.0,
        196.0,
        24.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Center,
        "-Choose Difficulty-",
    );
    fonts.draw_multiline(
        canvas,
        FontFamily::Neuropol,
        400.0,
        228.0,
        16.0,
        20.0,
        (0xC0, 0xC0, 0xC0),
        None,
        TextAlign::Center,
        360.0,
        "(Platinum Trophies are only available\nin Hardcore mode.)",
        1.0,
    );
    for (d, lbl, y) in [
        (Difficulty::Easy, "Relaxed (Easy)", 290.0),
        (Difficulty::Medium, "Casual (Medium)", 320.0),
        (Difficulty::Hard, "Hardcore (Hard)", 350.0),
    ] {
        draw_radio(canvas, fonts, 265.0, y, lbl, selected == d);
    }
    if ok_only {
        let r_ok = Rect::new(400.0 - 47.0, 396.0, 94.0, 53.0);
        draw_modal_button(
            canvas,
            bank,
            fonts,
            r_ok,
            "Ok",
            r_ok.contains(mouse_pos.0, mouse_pos.1),
            true,
        );
    } else {
        let r_p = Rect::new(298.0, 396.0, 94.0, 53.0);
        let r_b = Rect::new(408.0, 396.0, 94.0, 53.0);
        draw_modal_button(
            canvas,
            bank,
            fonts,
            r_p,
            "play",
            r_p.contains(mouse_pos.0, mouse_pos.1),
            true,
        );
        draw_modal_button(
            canvas,
            bank,
            fonts,
            r_b,
            "back",
            r_b.contains(mouse_pos.0, mouse_pos.1),
            true,
        );
    }
}

fn render_ingame_diff_modal(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    level_only: bool,
    mouse_pos: (f32, f32),
) {
    canvas.fill_rect_alpha(
        Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
        0,
        0,
        0,
        105,
    );
    draw_modal_bubble(canvas, Rect::new(200.0, 210.0, 400.0, 228.0));
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        400.0,
        226.0,
        24.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Center,
        "Hey- this one is tough!",
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        400.0,
        270.0,
        20.0,
        (255, 255, 255),
        TextAlign::Center,
        "Want to turn off the clock?",
    );
    draw_checkbox(
        canvas,
        fonts,
        285.0,
        326.0,
        "Only for this level",
        level_only,
        Rect::new(280.0, 325.0, 240.0, 24.0).contains(mouse_pos.0, mouse_pos.1),
    );
    let r_yes = Rect::new(298.0, 368.0, 94.0, 53.0);
    let r_no = Rect::new(408.0, 368.0, 94.0, 53.0);
    draw_modal_button(
        canvas,
        bank,
        fonts,
        r_yes,
        "Yes",
        r_yes.contains(mouse_pos.0, mouse_pos.1),
        true,
    );
    draw_modal_button(
        canvas,
        bank,
        fonts,
        r_no,
        "No",
        r_no.contains(mouse_pos.0, mouse_pos.1),
        true,
    );
}

fn render_question_modal(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    message: &str,
    yes_label: &str,
    no_label: Option<&str>,
    mouse_pos: (f32, f32),
) {
    canvas.fill_rect_alpha(
        Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
        0,
        0,
        0,
        105,
    );
    let lines = fonts.wrap_lines(FontFamily::Neuropol, message, 22.0, 370.0);
    let msg_h = (lines.len().max(1) as f32) * 26.0 + 15.0;
    let total_h = 15.0 + msg_h + 30.0 + 53.0 + 15.0;
    let bx = 200.0;
    let by = (600.0 - total_h) * 0.5;
    draw_modal_bubble(canvas, Rect::new(bx, by, 400.0, total_h));
    fonts.draw_multiline(
        canvas,
        FontFamily::Neuropol,
        400.0,
        by + 18.0,
        22.0,
        26.0,
        (255, 255, 255),
        None,
        TextAlign::Center,
        370.0,
        message,
        1.0,
    );

    let btn_y = by + 15.0 + msg_h + 30.0;
    if let Some(nl) = no_label {
        let r1 = Rect::new(400.0 - 99.0, btn_y, 94.0, 53.0);
        let r2 = Rect::new(405.0, btn_y, 94.0, 53.0);
        draw_modal_button(
            canvas,
            bank,
            fonts,
            r1,
            yes_label,
            r1.contains(mouse_pos.0, mouse_pos.1),
            true,
        );
        draw_modal_button(
            canvas,
            bank,
            fonts,
            r2,
            nl,
            r2.contains(mouse_pos.0, mouse_pos.1),
            true,
        );
    } else {
        let r1 = Rect::new(400.0 - 47.0, btn_y, 94.0, 53.0);
        draw_modal_button(
            canvas,
            bank,
            fonts,
            r1,
            yes_label,
            r1.contains(mouse_pos.0, mouse_pos.1),
            true,
        );
    }
}

fn render_award_modal(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    kind: AwardKind,
    mouse_pos: (f32, f32),
) {
    canvas.fill_rect_alpha(
        Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
        0,
        0,
        0,
        105,
    );
    let bx = 175.0;
    let by = 185.0;
    draw_modal_bubble(canvas, Rect::new(bx, by, 450.0, 260.0));
    if let Some((tid, bw, bh)) = bank.named(kind.bitmap_name()) {
        let w = bw as f32 * 0.7;
        let h = bh as f32 * 0.7;
        let x = bx + 18.0;
        let y = by + 24.0;
        if let Some((sh, sw, sh_h, pad)) =
            bank.glow_named(kind.bitmap_name(), GlowPreset::TrophyShadow)
        {
            let sx = w / bw.max(1) as f32;
            let sy = h / bh.max(1) as f32;
            canvas.draw_tex_rect(
                sh,
                Rect::new(
                    x - pad as f32 * sx,
                    y - pad as f32 * sy,
                    sw as f32 * sx,
                    sh_h as f32 * sy,
                ),
                None,
                1.0,
                1.0,
            );
        }
        canvas.draw_tex_rect(tid, Rect::new(x, y, w, h), None, 1.0, 1.0);
    }
    fonts.draw_multiline(
        canvas,
        FontFamily::Neuropol,
        bx + 265.0,
        by + 28.0,
        22.0,
        26.0,
        (0xFF, 0xFF, 0x40),
        None,
        TextAlign::Center,
        300.0,
        &format!("TROPHY EARNED:\n{}", kind.id()),
        1.0,
    );
    fonts.draw_multiline(
        canvas,
        FontFamily::Neuropol,
        bx + 265.0,
        by + 104.0,
        21.0,
        25.0,
        (255, 255, 255),
        None,
        TextAlign::Center,
        300.0,
        kind.notification(),
        1.0,
    );
    let r_ok = Rect::new(400.0 - 47.0, 378.0, 94.0, 53.0);
    draw_modal_button(
        canvas,
        bank,
        fonts,
        r_ok,
        "Ok",
        r_ok.contains(mouse_pos.0, mouse_pos.1),
        true,
    );
}

fn render_endless_setup_modal(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    cfg: EndlessConfig,
    mouse_pos: (f32, f32),
) {
    canvas.fill_rect_alpha(
        Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
        0,
        0,
        0,
        105,
    );
    draw_modal_bubble(canvas, Rect::new(185.0, 120.0, 430.0, 362.0));
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        400.0,
        138.0,
        24.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Center,
        "-Endless Mode-",
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        400.0,
        172.0,
        16.0,
        (0xC0, 0xD4, 0xF0),
        TextAlign::Center,
        "Choose difficulties to generate:",
    );

    let rows = [
        ("Easy", "(classic pipes)", cfg.easy, 204.0),
        ("Medium", "(portals & frost)", cfg.medium, 238.0),
        ("Hard", "(gates, magic & portals)", cfg.hard, 272.0),
    ];
    for (lbl, hint, checked, y) in rows {
        let hovered = Rect::new(225.0, y - 6.0, 355.0, 26.0).contains(mouse_pos.0, mouse_pos.1);
        draw_checkbox(canvas, fonts, 225.0, y, lbl, checked, hovered);
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            578.0,
            y + 1.0,
            13.0,
            (0xB8, 0xCA, 0xE6),
            TextAlign::Right,
            hint,
        );
    }

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        225.0,
        314.0,
        16.0,
        (255, 255, 255),
        TextAlign::Left,
        "Max Grid:",
    );
    let draw_step = |canvas: &mut Canvas, x_center: f32, rect: Rect, label: &str, enabled: bool| {
        let hov = enabled && rect.contains(mouse_pos.0, mouse_pos.1);
        draw_text_button(
            canvas,
            fonts,
            x_center,
            314.0,
            16.0,
            TextAlign::Center,
            label,
            hov,
            None,
            if enabled { 1.0 } else { 0.35 },
        );
    };
    draw_step(
        canvas,
        405.0,
        Rect::new(392.0, 312.0, 26.0, 24.0),
        "[-]",
        cfg.max_cols > MIN_GEN_COLS,
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        434.0,
        314.0,
        16.0,
        (255, 255, 255),
        TextAlign::Center,
        &cfg.max_cols.to_string(),
    );
    draw_step(
        canvas,
        463.0,
        Rect::new(450.0, 312.0, 26.0, 24.0),
        "[+]",
        cfg.max_cols < MAX_GRID_COLS,
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        484.0,
        315.0,
        15.0,
        (0xC0, 0xD4, 0xF0),
        TextAlign::Center,
        "x",
    );
    draw_step(
        canvas,
        509.0,
        Rect::new(496.0, 312.0, 26.0, 24.0),
        "[-]",
        cfg.max_rows > MIN_GEN_ROWS,
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        538.0,
        314.0,
        16.0,
        (255, 255, 255),
        TextAlign::Center,
        &cfg.max_rows.to_string(),
    );
    draw_step(
        canvas,
        567.0,
        Rect::new(554.0, 312.0, 26.0, 24.0),
        "[+]",
        cfg.max_rows < MAX_GRID_ROWS,
    );

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        225.0,
        352.0,
        16.0,
        (255, 255, 255),
        TextAlign::Left,
        "Play Rules:",
    );
    let r_rules = Rect::new(370.0, 348.0, 212.0, 26.0);
    draw_text_button(
        canvas,
        fonts,
        578.0,
        352.0,
        16.0,
        TextAlign::Right,
        cfg.rules.label(),
        r_rules.contains(mouse_pos.0, mouse_pos.1),
        Some((0x42, 0x83, 0xF3)),
        1.0,
    );

    if cfg.any_enabled() {
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            400.0,
            384.0,
            13.0,
            (0xB8, 0xCA, 0xE6),
            TextAlign::Center,
            "Scores & trophies are shown per run and not saved.",
        );
    } else {
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            400.0,
            384.0,
            13.0,
            (0xFF, 0x9A, 0x8C),
            TextAlign::Center,
            "(Select at least one difficulty)",
        );
    }

    let r_play = Rect::new(298.0, 412.0, 94.0, 53.0);
    let r_back = Rect::new(408.0, 412.0, 94.0, 53.0);
    draw_modal_button(
        canvas,
        bank,
        fonts,
        r_play,
        "play",
        r_play.contains(mouse_pos.0, mouse_pos.1),
        cfg.any_enabled(),
    );
    draw_modal_button(
        canvas,
        bank,
        fonts,
        r_back,
        "back",
        r_back.contains(mouse_pos.0, mouse_pos.1),
        true,
    );
}
