// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::Difficulty;
use crate::save::SaveData;
use crate::ui::widgets::UiAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleSubmenu {
    Main,
    PlayChoices,
    DifficultyChoices,
}

pub const BUTTON_RIGHT: f32 = 520.0;
pub const BUTTON_Y: f32 = 270.0;
pub const BUTTON_SPACING: f32 = 30.0;
pub const INFO_GUTTER: f32 = 15.0;

pub const TICKER_TEXT: &str = "                        Aqualux Copyright (C) 2010 Milkman Games, LLC.  All Rights Reserved.       By: Alex Liebert.      Server programming: Micheal Courtney-Brooks.     Have you created your own level yet?    ";

pub struct MenuEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub info: &'static str,
    pub action: Option<UiAction>,
}

pub fn menu_entries(submenu: TitleSubmenu, save: &SaveData) -> Vec<MenuEntry> {
    let has_save = save.active_profile().is_some_and(|p| p.has_saved_game());
    let match_unlocked = save.active_profile().is_some_and(|p| p.is_match_unlocked());
    let mirror_unlocked = save
        .active_profile()
        .is_some_and(|p| p.is_mirror_unlocked());

    match submenu {
        TitleSubmenu::Main => vec![
            MenuEntry {
                id: "btn_start",
                label: "start game",
                info: "Play the campaign or user levels from \n around the globe!",
                action: Some(UiAction::OpenPlayMenu),
            },
            MenuEntry {
                id: "btn_levelmaker",
                label: "levelmaker",
                info: "Create your own levels to share with \n aqualuxers around the world! \n \n *internet connection required",
                action: Some(UiAction::OpenLevelmakerNotice),
            },
            MenuEntry {
                id: "btn_trophy",
                label: "trophy room",
                info: "Check trophies you've earned or \n retry for higher scores!",
                action: Some(UiAction::OpenTrophyRoom),
            },
            MenuEntry {
                id: "btn_options",
                label: "options",
                info: "Change game settings.",
                action: Some(UiAction::OpenOptions),
            },
            MenuEntry {
                id: "btn_quit",
                label: "quit",
                info: "quit to desktop.",
                action: Some(UiAction::QuitPrompt),
            },
        ],
        TitleSubmenu::PlayChoices => {
            let mut out = if has_save {
                vec![
                    MenuEntry {
                        id: "btn_continue",
                        label: "continue",
                        info: "Continue from where you left off.",
                        action: Some(UiAction::ContinueCampaign),
                    },
                    MenuEntry {
                        id: "btn_start_over",
                        label: "start over",
                        info: "Start a new game.",
                        action: Some(UiAction::StartOverPrompt),
                    },
                    MenuEntry {
                        id: "btn_lotd",
                        label: "levels of the day",
                        info: "Play levels created by other aqualuxers - \n new levels added all the time! \n \n *internet connection required",
                        action: Some(UiAction::PlayLotd),
                    },
                ]
            } else {
                vec![
                    MenuEntry {
                        id: "btn_new_game",
                        label: "new game",
                        info: "Start a new game in story mode.",
                        action: Some(UiAction::StartOverPrompt),
                    },
                    MenuEntry {
                        id: "btn_lotd",
                        label: "levels of the day",
                        info: "Play levels created by other aqualuxers - \n new levels added all the time! \n \n *internet connection required",
                        action: Some(UiAction::PlayLotd),
                    },
                ]
            };
            if mirror_unlocked {
                out.push(MenuEntry {
                    id: "btn_mirror",
                    label: "mirror match mode",
                    info: "Spot the differences between \n two mirrored grids.",
                    action: Some(UiAction::StartMatch),
                });
            } else if match_unlocked {
                out.push(MenuEntry {
                    id: "btn_match",
                    label: "match mode",
                    info: "Spot the differences between two grids.",
                    action: Some(UiAction::StartMatch),
                });
            }
            out.push(MenuEntry {
                id: "",
                label: "",
                info: "",
                action: None,
            });
            out.push(MenuEntry {
                id: "btn_back_main",
                label: "< back",
                info: "",
                action: Some(UiAction::BackToMainMenu),
            });
            out
        }
        TitleSubmenu::DifficultyChoices => vec![
            MenuEntry {
                id: "btn_easy",
                label: "relaxed (easy)",
                info: "Take it easy- no time limits at all.",
                action: Some(UiAction::ConfirmDifficultyCampaign(Difficulty::Easy)),
            },
            MenuEntry {
                id: "btn_med",
                label: "casual (normal)",
                info: "Medium difficulty.",
                action: Some(UiAction::ConfirmDifficultyCampaign(Difficulty::Medium)),
            },
            MenuEntry {
                id: "btn_hard",
                label: "hardcore (hard)",
                info: "You play a lot of puzzlers \n and you're ready for a challenge!",
                action: Some(UiAction::ConfirmDifficultyCampaign(Difficulty::Hard)),
            },
            MenuEntry {
                id: "",
                label: "",
                info: "",
                action: None,
            },
            MenuEntry {
                id: "btn_back_play",
                label: "< back",
                info: "",
                action: Some(UiAction::BackToPlayMenu),
            },
        ],
    }
}
