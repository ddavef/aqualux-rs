// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::AssetBundle;
use crate::game::generator::{GenDifficulty, MIN_GEN_COLS, MIN_GEN_ROWS};
use crate::game::level_io::{MAX_GRID_COLS, MAX_GRID_ROWS};
use crate::game::{AwardKind, Difficulty};
use crate::render::hdr::{HDR_PAPER_WHITE_MAX, HDR_PAPER_WHITE_MIN, HDR_PEAK_MAX, HDR_PEAK_MIN};
use crate::render::{
    Canvas, FontFamily, FontSet, GlowPreset, LOGICAL_H, LOGICAL_W, Rect, TexBank, TextAlign,
};
use crate::save::SaveData;
use crate::ui::editor::{build_level_props_buttons, render_level_props_modal};
use crate::ui::profiles::{
    build_enter_name_buttons, build_profile_modal_buttons, render_enter_name_modal,
    render_profile_modal,
};
use crate::ui::trophy_room::{build_trophy_room_buttons, render_trophy_room};
use crate::ui::widgets::{
    ModalState, UiAction, UiButton, draw_checkbox, draw_checkbox_enabled, draw_modal_bubble,
    draw_modal_button, draw_plain_text_button, draw_radio, draw_slider, draw_slider_enabled,
    draw_text_button,
};
use crate::ui::{EndlessConfig, EndlessElement};

pub const OPTIONS_SLIDER_X: f32 = 435.0;

#[derive(Debug, Clone, Copy)]
pub struct OptionsLayout {
    pub bx: f32,
    pub base_y: f32,
    pub bh: f32,
    pub fx_y: f32,
    pub music_y: f32,
    pub diff_y: f32,
    pub cb_y: f32,
    pub win_y: f32,
    pub hdr_y: f32,
    pub hdr_white_y: Option<f32>,
    pub hdr_peak_y: Option<f32>,
    pub fps_y: f32,
    pub done_y: f32,
}

#[cfg(test)]
pub fn options_layout(allow_difficulty: bool) -> OptionsLayout {
    options_layout_for(allow_difficulty, false)
}

pub fn options_layout_for(allow_difficulty: bool, show_hdr_calibration: bool) -> OptionsLayout {
    let bx = 215.0;
    let diff_off = 125.0;
    let cb_off = if allow_difficulty { 168.0 } else { 138.0 };
    let win_off = cb_off + 30.0;
    let hdr_off = win_off + 30.0;
    let (white_off, peak_off, fps_off) = if show_hdr_calibration {
        (Some(hdr_off + 30.0), Some(hdr_off + 60.0), hdr_off + 92.0)
    } else {
        (None, None, hdr_off + 30.0)
    };
    let done_off = fps_off + 38.0;
    let bh = done_off + 53.0 + 18.0;
    let base_y = (LOGICAL_H as f32 - bh) * 0.5;
    OptionsLayout {
        bx,
        base_y,
        bh,
        fx_y: base_y + 65.0,
        music_y: base_y + 95.0,
        diff_y: base_y + diff_off,
        cb_y: base_y + cb_off,
        win_y: base_y + win_off,
        hdr_y: base_y + hdr_off,
        hdr_white_y: white_off.map(|o| base_y + o),
        hdr_peak_y: peak_off.map(|o| base_y + o),
        fps_y: base_y + fps_off,
        done_y: base_y + done_off,
    }
}

const DIFF_BY: f32 = (LOGICAL_H as f32 - 285.0) * 0.5;
const IGD_BY: f32 = (LOGICAL_H as f32 - 228.0) * 0.5;
const AWARD_BY: f32 = (LOGICAL_H as f32 - 260.0) * 0.5;

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
    pause_elapsed: f32,
) {
    canvas.fill_rect_alpha(
        Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
        0,
        0,
        0,
        26,
    );
    let menu_alpha = crate::render::easing::quad_out((pause_elapsed / 1.0).clamp(0.0, 1.0));
    let start = canvas.cmds.len();
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        400.0,
        180.0,
        45.0,
        (0xF4, 0xF7, 0xF7),
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
    canvas.transform_cmds_since(start, 400.0, 300.0, 1.0, menu_alpha);
}

pub fn build_modal_buttons(
    modal: &ModalState,
    assets: &AssetBundle,
    bank: &TexBank,
    fonts: &FontSet,
    save: &SaveData,
    hdr_available: bool,
    hdr_manual_calibration: bool,
) -> Vec<UiButton> {
    match modal {
        ModalState::TrophyRoom { page, .. } => build_trophy_room_buttons(assets, bank, save, *page),
        ModalState::Options { allow_difficulty } => {
            let lay =
                options_layout_for(*allow_difficulty, hdr_available && hdr_manual_calibration);
            let mut out = Vec::new();
            if *allow_difficulty {
                out.push(UiButton {
                    id: "opt_diff",
                    rect: Rect::new(OPTIONS_SLIDER_X - 55.0, lay.diff_y, 175.0, 24.0),
                    action: UiAction::OpenOptionsDifficulty,
                    enabled: true,
                    hover_sfx: Some("PopTwoSound"),
                });
            }
            out.push(UiButton {
                id: "opt_cb",
                rect: Rect::new(lay.bx + 20.0, lay.cb_y, 260.0, 22.0),
                action: UiAction::ToggleColorBlind,
                enabled: true,
                hover_sfx: None,
            });
            out.push(UiButton {
                id: "opt_win",
                rect: Rect::new(lay.bx + 20.0, lay.win_y, 260.0, 22.0),
                action: UiAction::ToggleWindowed,
                enabled: true,
                hover_sfx: None,
            });
            out.push(UiButton {
                id: "opt_hdr",
                rect: Rect::new(lay.bx + 20.0, lay.hdr_y, 330.0, 22.0),
                action: UiAction::ToggleHdr,
                enabled: hdr_available,
                hover_sfx: None,
            });
            out.push(UiButton {
                id: "opt_fps",
                rect: Rect::new(OPTIONS_SLIDER_X - 20.0, lay.fps_y - 3.0, 145.0, 24.0),
                action: UiAction::CycleFps,
                enabled: true,
                hover_sfx: Some("PopTwoSound"),
            });
            out.push(UiButton {
                id: "opt_done",
                rect: Rect::new(400.0 - 47.0, lay.done_y, 94.0, 53.0),
                action: UiAction::OptionsDone,
                enabled: true,
                hover_sfx: None,
            });
            out
        }
        ModalState::Difficulty { ok_only, .. } => {
            let btn_y = DIFF_BY + 216.0;
            let mut out = vec![
                UiButton {
                    id: "d_easy",
                    rect: Rect::new(265.0, DIFF_BY + 110.0, 260.0, 26.0),
                    action: UiAction::SelectDifficultyRadio(Difficulty::Easy),
                    enabled: true,
                    hover_sfx: None,
                },
                UiButton {
                    id: "d_med",
                    rect: Rect::new(265.0, DIFF_BY + 140.0, 260.0, 26.0),
                    action: UiAction::SelectDifficultyRadio(Difficulty::Medium),
                    enabled: true,
                    hover_sfx: None,
                },
                UiButton {
                    id: "d_hard",
                    rect: Rect::new(265.0, DIFF_BY + 170.0, 260.0, 26.0),
                    action: UiAction::SelectDifficultyRadio(Difficulty::Hard),
                    enabled: true,
                    hover_sfx: None,
                },
            ];
            if *ok_only {
                out.push(UiButton {
                    id: "d_ok",
                    rect: Rect::new(400.0 - 47.0, btn_y, 94.0, 53.0),
                    action: UiAction::ConfirmDifficultyModal,
                    enabled: true,
                    hover_sfx: None,
                });
            } else {
                out.push(UiButton {
                    id: "d_play",
                    rect: Rect::new(301.0, btn_y, 94.0, 53.0),
                    action: UiAction::ConfirmDifficultyModal,
                    enabled: true,
                    hover_sfx: None,
                });
                out.push(UiButton {
                    id: "d_back",
                    rect: Rect::new(405.0, btn_y, 94.0, 53.0),
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
                rect: Rect::new(280.0, IGD_BY + 115.0, 240.0, 24.0),
                action: UiAction::ToggleLevelOnlyCheckbox,
                enabled: true,
                hover_sfx: None,
            },
            UiButton {
                id: "igd_yes",
                rect: Rect::new(301.0, IGD_BY + 158.0, 94.0, 53.0),
                action: UiAction::InGameDiffYes,
                enabled: true,
                hover_sfx: None,
            },
            UiButton {
                id: "igd_no",
                rect: Rect::new(405.0, IGD_BY + 158.0, 94.0, 53.0),
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
            rect: Rect::new(400.0 - 47.0, AWARD_BY + 193.0, 94.0, 53.0),
            action: UiAction::DismissAward,
            enabled: true,
            hover_sfx: None,
        }],
        ModalState::LevelProps(props) => build_level_props_buttons(props),
        ModalState::EndlessSetup(cfg) => {
            let mut buttons = vec![
                UiButton {
                    id: "end_easy",
                    rect: Rect::new(84.0, 171.0, 304.0, 24.0),
                    action: UiAction::EndlessToggleDifficulty(GenDifficulty::Easy),
                    enabled: !cfg.custom,
                    hover_sfx: None,
                },
                UiButton {
                    id: "end_med",
                    rect: Rect::new(84.0, 201.0, 304.0, 24.0),
                    action: UiAction::EndlessToggleDifficulty(GenDifficulty::Medium),
                    enabled: !cfg.custom,
                    hover_sfx: None,
                },
                UiButton {
                    id: "end_hard",
                    rect: Rect::new(84.0, 231.0, 304.0, 24.0),
                    action: UiAction::EndlessToggleDifficulty(GenDifficulty::Hard),
                    enabled: !cfg.custom,
                    hover_sfx: None,
                },
                UiButton {
                    id: "end_custom",
                    rect: Rect::new(84.0, 261.0, 304.0, 24.0),
                    action: UiAction::EndlessToggleCustom,
                    enabled: true,
                    hover_sfx: None,
                },
                UiButton {
                    id: "end_cols_dec",
                    rect: Rect::new(203.0, 308.0, 26.0, 24.0),
                    action: UiAction::EndlessAdjustMaxCols(-1),
                    enabled: cfg.max_cols > MIN_GEN_COLS,
                    hover_sfx: Some("PopTwoSound"),
                },
                UiButton {
                    id: "end_cols_inc",
                    rect: Rect::new(259.0, 308.0, 26.0, 24.0),
                    action: UiAction::EndlessAdjustMaxCols(1),
                    enabled: cfg.max_cols < MAX_GRID_COLS,
                    hover_sfx: Some("PopTwoSound"),
                },
                UiButton {
                    id: "end_rows_dec",
                    rect: Rect::new(299.0, 308.0, 26.0, 24.0),
                    action: UiAction::EndlessAdjustMaxRows(-1),
                    enabled: cfg.max_rows > MIN_GEN_ROWS,
                    hover_sfx: Some("PopTwoSound"),
                },
                UiButton {
                    id: "end_rows_inc",
                    rect: Rect::new(355.0, 308.0, 26.0, 24.0),
                    action: UiAction::EndlessAdjustMaxRows(1),
                    enabled: cfg.max_rows < MAX_GRID_ROWS,
                    hover_sfx: Some("PopTwoSound"),
                },
                UiButton {
                    id: "end_rules",
                    rect: Rect::new(178.0, 344.0, 212.0, 26.0),
                    action: UiAction::EndlessCycleRules,
                    enabled: true,
                    hover_sfx: Some("PopTwoSound"),
                },
                UiButton {
                    id: "end_play",
                    rect: Rect::new(301.0, 436.0, 94.0, 53.0),
                    action: UiAction::StartEndlessRun,
                    enabled: cfg.any_enabled(),
                    hover_sfx: None,
                },
                UiButton {
                    id: "end_back",
                    rect: Rect::new(405.0, 436.0, 94.0, 53.0),
                    action: UiAction::CloseTopModal,
                    enabled: true,
                    hover_sfx: None,
                },
            ];
            let el_rows = [
                ("end_el_mov", EndlessElement::Movable),
                ("end_el_rot", EndlessElement::Rotatable),
                ("end_el_imm", EndlessElement::Immovable),
                ("end_el_frz", EndlessElement::Frosted),
                ("end_el_rock", EndlessElement::Rocks),
                ("end_el_gm", EndlessElement::GateMagic),
                ("end_el_port", EndlessElement::Portals),
            ];
            for (idx, (id, el)) in el_rows.into_iter().enumerate() {
                let y = 176.0 + (idx + 2) as f32 * 26.0;
                buttons.push(UiButton {
                    id,
                    rect: Rect::new(428.0, y - 4.0, 288.0, 23.0),
                    action: UiAction::EndlessToggleElement(el),
                    enabled: cfg.custom,
                    hover_sfx: None,
                });
            }
            buttons
        }
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
    modal_ages: &[f32],
    trophy_hover_age: f32,
    options_windowed_checked: bool,
    options_hdr_checked: bool,
    hdr_available: bool,
    hdr_manual_calibration: bool,
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
        canvas.fill_rect_alpha(
            Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
            0,
            0,
            0,
            105,
        );
        let age = modal_ages.get(idx).copied().unwrap_or(1.0);
        let t = (age / 0.52).clamp(0.0, 1.0);
        let eased = crate::render::easing::back_out(t);
        let start = canvas.cmds.len();
        match m {
            ModalState::TrophyRoom { page, .. } => {
                let h_age = if idx == last_idx {
                    trophy_hover_age
                } else {
                    0.0
                };
                render_trophy_room(canvas, assets, bank, fonts, save, *page, mpos, h_age);
            }
            ModalState::Options { allow_difficulty } => {
                render_options_modal(
                    canvas,
                    bank,
                    fonts,
                    save,
                    *allow_difficulty,
                    options_windowed_checked,
                    options_hdr_checked,
                    hdr_available,
                    hdr_manual_calibration,
                    mpos,
                );
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
        if matches!(m, ModalState::TrophyRoom { .. }) {
            canvas.transform_cmds_since(start, 400.0, 300.0, 1.0, eased.clamp(0.0, 1.0));
        } else {
            let pop_scale = (0.1 + 0.9 * eased).max(0.01);
            let pop_alpha = (0.1 + 0.9 * eased).clamp(0.0, 1.0);
            canvas.transform_cmds_since(start, 400.0, 300.0, pop_scale, pop_alpha);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_options_modal(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    save: &SaveData,
    allow_difficulty: bool,
    windowed_checked: bool,
    hdr_checked: bool,
    hdr_available: bool,
    hdr_manual_calibration: bool,
    mouse_pos: (f32, f32),
) {
    let show_cal = hdr_available && hdr_manual_calibration;
    let lay = options_layout_for(allow_difficulty, show_cal);
    draw_modal_bubble(canvas, Rect::new(lay.bx, lay.base_y, 370.0, lay.bh));

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        400.0,
        lay.base_y + 16.0,
        22.0,
        (255, 255, 255),
        TextAlign::Center,
        "-Options-",
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        lay.bx + 20.0,
        lay.fx_y,
        17.0,
        (255, 255, 255),
        TextAlign::Left,
        "fx volume:",
    );
    draw_slider(canvas, OPTIONS_SLIDER_X, lay.fx_y, save.config.sfx_volume);
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        lay.bx + 20.0,
        lay.music_y,
        17.0,
        (255, 255, 255),
        TextAlign::Left,
        "music volume:",
    );
    draw_slider(
        canvas,
        OPTIONS_SLIDER_X,
        lay.music_y,
        save.config.music_volume,
    );

    if allow_difficulty {
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            lay.bx + 20.0,
            lay.diff_y,
            17.0,
            (255, 255, 255),
            TextAlign::Left,
            "set difficulty:",
        );
        let dlbl = save
            .active_profile()
            .map(|p| p.difficulty.human_label())
            .unwrap_or("casual (medium)");
        let r_df = Rect::new(OPTIONS_SLIDER_X - 55.0, lay.diff_y, 175.0, 24.0);
        draw_plain_text_button(
            canvas,
            fonts,
            OPTIONS_SLIDER_X + 120.0,
            lay.diff_y + 1.0,
            16.0,
            TextAlign::Right,
            dlbl,
            r_df.contains(mouse_pos.0, mouse_pos.1),
            (0x42, 0x83, 0xB3),
        );
    }

    draw_checkbox(
        canvas,
        fonts,
        lay.bx + 20.0,
        lay.cb_y,
        "colorblind mode",
        save.config.color_blind,
        Rect::new(lay.bx + 20.0, lay.cb_y, 260.0, 22.0).contains(mouse_pos.0, mouse_pos.1),
    );
    draw_checkbox(
        canvas,
        fonts,
        lay.bx + 20.0,
        lay.win_y,
        "windowed mode",
        windowed_checked,
        Rect::new(lay.bx + 20.0, lay.win_y, 260.0, 22.0).contains(mouse_pos.0, mouse_pos.1),
    );
    let hdr_lbl = if hdr_available {
        "hdr mode"
    } else {
        "hdr mode (no hdr)"
    };
    draw_checkbox_enabled(
        canvas,
        fonts,
        lay.bx + 20.0,
        lay.hdr_y,
        hdr_lbl,
        hdr_available && hdr_checked,
        hdr_available
            && Rect::new(lay.bx + 20.0, lay.hdr_y, 330.0, 22.0).contains(mouse_pos.0, mouse_pos.1),
        hdr_available,
    );
    if show_cal && let (Some(wy), Some(py)) = (lay.hdr_white_y, lay.hdr_peak_y) {
        let cal_col = if hdr_checked {
            (255, 255, 255)
        } else {
            (155, 165, 180)
        };
        let white_nits = save
            .config
            .hdr_paper_white_nits
            .clamp(HDR_PAPER_WHITE_MIN, HDR_PAPER_WHITE_MAX);
        let white_01 =
            (white_nits - HDR_PAPER_WHITE_MIN) / (HDR_PAPER_WHITE_MAX - HDR_PAPER_WHITE_MIN);
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            lay.bx + 20.0,
            wy + 1.0,
            15.0,
            cal_col,
            TextAlign::Left,
            &format!("hdr white ({:.0})", white_nits),
        );
        draw_slider_enabled(canvas, OPTIONS_SLIDER_X, wy, white_01, hdr_checked);

        let peak_nits = save.config.hdr_peak_nits.clamp(HDR_PEAK_MIN, HDR_PEAK_MAX);
        let peak_01 = (peak_nits - HDR_PEAK_MIN) / (HDR_PEAK_MAX - HDR_PEAK_MIN);
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            lay.bx + 20.0,
            py + 1.0,
            15.0,
            cal_col,
            TextAlign::Left,
            &format!("hdr peak ({:.0})", peak_nits),
        );
        draw_slider_enabled(canvas, OPTIONS_SLIDER_X, py, peak_01, hdr_checked);
    }

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        lay.bx + 20.0,
        lay.fps_y - 2.0,
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
    let r_fps = Rect::new(OPTIONS_SLIDER_X - 20.0, lay.fps_y - 3.0, 145.0, 24.0);
    draw_plain_text_button(
        canvas,
        fonts,
        OPTIONS_SLIDER_X + 120.0,
        lay.fps_y - 2.0,
        16.0,
        TextAlign::Right,
        &fps_lbl,
        r_fps.contains(mouse_pos.0, mouse_pos.1),
        (0x42, 0x83, 0xB3),
    );

    let r_done = Rect::new(400.0 - 47.0, lay.done_y, 94.0, 53.0);
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
    draw_modal_bubble(canvas, Rect::new(205.0, DIFF_BY, 390.0, 285.0));
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        400.0,
        DIFF_BY + 16.0,
        24.0,
        (255, 255, 255),
        TextAlign::Center,
        "-Choose Difficulty-",
    );
    fonts.draw_multiline(
        canvas,
        FontFamily::Neuropol,
        400.0,
        DIFF_BY + 48.0,
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
        (Difficulty::Easy, "Relaxed (Easy)", DIFF_BY + 110.0),
        (Difficulty::Medium, "Casual (Medium)", DIFF_BY + 140.0),
        (Difficulty::Hard, "Hardcore (Hard)", DIFF_BY + 170.0),
    ] {
        draw_radio(canvas, fonts, 265.0, y, lbl, selected == d);
    }
    let btn_y = DIFF_BY + 216.0;
    if ok_only {
        let r_ok = Rect::new(400.0 - 47.0, btn_y, 94.0, 53.0);
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
        let r_p = Rect::new(301.0, btn_y, 94.0, 53.0);
        let r_b = Rect::new(405.0, btn_y, 94.0, 53.0);
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
    draw_modal_bubble(canvas, Rect::new(200.0, IGD_BY, 400.0, 228.0));
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        400.0,
        IGD_BY + 16.0,
        24.0,
        (255, 255, 255),
        TextAlign::Center,
        "Hey- this one is tough!",
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        400.0,
        IGD_BY + 60.0,
        20.0,
        (255, 255, 255),
        TextAlign::Center,
        "Want to turn off the clock?",
    );
    draw_checkbox(
        canvas,
        fonts,
        285.0,
        IGD_BY + 116.0,
        "Only for this level",
        level_only,
        Rect::new(280.0, IGD_BY + 115.0, 240.0, 24.0).contains(mouse_pos.0, mouse_pos.1),
    );
    let btn_y = IGD_BY + 158.0;
    let r_yes = Rect::new(301.0, btn_y, 94.0, 53.0);
    let r_no = Rect::new(405.0, btn_y, 94.0, 53.0);
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
    let bx = 175.0;
    let by = AWARD_BY;
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
        if kind != AwardKind::Mystery {
            canvas.sheen_last_sprite(1.38);
        }
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
    let r_ok = Rect::new(400.0 - 47.0, by + 193.0, 94.0, 53.0);
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
    draw_modal_bubble(canvas, Rect::new(60.0, 96.0, 680.0, 408.0));
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        400.0,
        112.0,
        24.0,
        (255, 255, 255),
        TextAlign::Center,
        "-Endless Mode-",
    );
    canvas.draw_line(408.0, 148.0, 408.0, 406.0, 1.5, (0x7E, 0xAD, 0xCF, 100));

    let diff_hdr_col = if cfg.custom {
        (145, 160, 182)
    } else {
        (0xC0, 0xD4, 0xF0)
    };
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        84.0,
        148.0,
        15.0,
        diff_hdr_col,
        TextAlign::Left,
        "Presets:",
    );

    let diff_rows = [
        (
            "Easy",
            "(classic)",
            !cfg.custom && cfg.easy,
            !cfg.custom,
            176.0,
        ),
        (
            "Medium",
            "(portals & frost)",
            !cfg.custom && cfg.medium,
            !cfg.custom,
            206.0,
        ),
        (
            "Hard",
            "(gates & magic)",
            !cfg.custom && cfg.hard,
            !cfg.custom,
            236.0,
        ),
        ("Custom", "(choose elements)", cfg.custom, true, 266.0),
    ];
    for (lbl, hint, checked, enabled, y) in diff_rows {
        let hovered =
            enabled && Rect::new(84.0, y - 5.0, 304.0, 24.0).contains(mouse_pos.0, mouse_pos.1);
        draw_checkbox_enabled(canvas, fonts, 84.0, y, lbl, checked, hovered, enabled);
        let hint_col = if enabled {
            (0xB8, 0xCA, 0xE6)
        } else {
            (145, 160, 182)
        };
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            388.0,
            y + 1.0,
            13.0,
            hint_col,
            TextAlign::Right,
            hint,
        );
    }

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        84.0,
        310.0,
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
            310.0,
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
        216.0,
        Rect::new(203.0, 308.0, 26.0, 24.0),
        "[-]",
        cfg.max_cols > MIN_GEN_COLS,
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        244.0,
        310.0,
        16.0,
        (255, 255, 255),
        TextAlign::Center,
        &cfg.max_cols.to_string(),
    );
    draw_step(
        canvas,
        272.0,
        Rect::new(259.0, 308.0, 26.0, 24.0),
        "[+]",
        cfg.max_cols < MAX_GRID_COLS,
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        292.0,
        311.0,
        15.0,
        (0xC0, 0xD4, 0xF0),
        TextAlign::Center,
        "x",
    );
    draw_step(
        canvas,
        312.0,
        Rect::new(299.0, 308.0, 26.0, 24.0),
        "[-]",
        cfg.max_rows > MIN_GEN_ROWS,
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        340.0,
        310.0,
        16.0,
        (255, 255, 255),
        TextAlign::Center,
        &cfg.max_rows.to_string(),
    );
    draw_step(
        canvas,
        368.0,
        Rect::new(355.0, 308.0, 26.0, 24.0),
        "[+]",
        cfg.max_rows < MAX_GRID_ROWS,
    );

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        84.0,
        348.0,
        16.0,
        (255, 255, 255),
        TextAlign::Left,
        "Rules:",
    );
    let r_rules = Rect::new(178.0, 344.0, 212.0, 26.0);
    draw_plain_text_button(
        canvas,
        fonts,
        388.0,
        348.0,
        16.0,
        TextAlign::Right,
        cfg.rules.label(),
        r_rules.contains(mouse_pos.0, mouse_pos.1),
        (0x42, 0x83, 0xB3),
    );

    let el_hdr_col = if cfg.custom {
        (0xC0, 0xD4, 0xF0)
    } else {
        (145, 160, 182)
    };
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        428.0,
        148.0,
        15.0,
        el_hdr_col,
        TextAlign::Left,
        "Board Elements:",
    );
    if !cfg.custom {
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            716.0,
            150.0,
            12.0,
            (145, 160, 182),
            TextAlign::Right,
            "(Custom only)",
        );
    }

    let el_rows = [
        ("Source", true, false, Some("(required)")),
        ("Drain", true, false, Some("(required)")),
        ("Movable pipes", cfg.elements.movable, cfg.custom, None),
        ("Rotatable pipes", cfg.elements.rotatable, cfg.custom, None),
        ("Immovable pipes", cfg.elements.immovable, cfg.custom, None),
        ("Frosted pipes", cfg.elements.frosted, cfg.custom, None),
        ("Rocks", cfg.elements.rocks, cfg.custom, None),
        ("Gate & Magic", cfg.elements.gate_magic, cfg.custom, None),
        ("Portals", cfg.elements.portals, cfg.custom, None),
    ];
    for (idx, (lbl, checked, enabled, note)) in el_rows.into_iter().enumerate() {
        let y = 176.0 + idx as f32 * 26.0;
        let hovered =
            enabled && Rect::new(428.0, y - 4.0, 288.0, 23.0).contains(mouse_pos.0, mouse_pos.1);
        draw_checkbox_enabled(canvas, fonts, 428.0, y, lbl, checked, hovered, enabled);
        if let Some(tag) = note {
            fonts.draw(
                canvas,
                FontFamily::Neuropol,
                716.0,
                y + 1.0,
                12.0,
                (145, 160, 182),
                TextAlign::Right,
                tag,
            );
        }
    }

    let (status_msg, status_col) = if cfg.any_enabled() {
        (
            "Scores & trophies are shown per run and not saved.",
            (0xB8, 0xCA, 0xE6),
        )
    } else if cfg.custom {
        ("(Enable Movable or Rotatable pipes)", (0xFF, 0x9A, 0x8C))
    } else {
        ("(Select at least one preset)", (0xFF, 0x9A, 0x8C))
    };
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        400.0,
        414.0,
        13.0,
        status_col,
        TextAlign::Center,
        status_msg,
    );

    let r_play = Rect::new(301.0, 436.0, 94.0, 53.0);
    let r_back = Rect::new(405.0, 436.0, 94.0, 53.0);
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
