// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::Difficulty;
use crate::render::BoardLayout;
use crate::ui::hud;
use crate::ui::summary::SummaryHideAction;
use crate::ui::title::TitleSubmenu;
use crate::ui::widgets::{LaunchSource, ModalState, QuestionId, UiAction};
use crate::ui::{GameApp, Screen};

const FPS_CYCLE: &[u32] = &[30, 60, 120, 144, 240, 0];

impl GameApp {
    pub(crate) fn trigger_action(&mut self, action: UiAction) {
        if !matches!(
            action,
            UiAction::SetMusicVol(_)
                | UiAction::SetSfxVol(_)
                | UiAction::ProfileSelectIdx(_)
                | UiAction::SelectDifficultyRadio(_)
                | UiAction::ToggleLevelOnlyCheckbox
                | UiAction::ToggleColorBlind
                | UiAction::ToggleWindowed
                | UiAction::NextTutorial
        ) {
            self.audio.play_sfx("ClickOneSound");
        }

        match action {
            UiAction::OpenPlayMenu => {
                self.title_submenu = TitleSubmenu::PlayChoices;
                self.title_submenu_age = 0.0;
            }
            UiAction::BackToMainMenu => {
                self.title_submenu = TitleSubmenu::Main;
                self.title_submenu_age = 0.0;
            }
            UiAction::BackToPlayMenu => {
                self.title_submenu = TitleSubmenu::PlayChoices;
                self.title_submenu_age = 0.0;
            }
            UiAction::StartOverPrompt => {
                let has_save = self
                    .save
                    .active_profile()
                    .is_some_and(|p| p.has_saved_game());
                if has_save {
                    self.modals.push(ModalState::Question {
                        id: QuestionId::ConfirmNewGame,
                        message: "Really start a new game? \n \n  Saved progress will be lost."
                            .to_string(),
                        yes_label: "yes",
                        no_label: Some("no"),
                    });
                } else {
                    self.title_submenu = TitleSubmenu::DifficultyChoices;
                    self.title_submenu_age = 0.0;
                }
            }
            UiAction::ConfirmDifficultyCampaign(d) => {
                self.start_title_hide(LaunchSource::NewCampaign(d))
            }
            UiAction::ContinueCampaign => self.start_title_hide(LaunchSource::ContinueCampaign),
            UiAction::PlayLotd => self.start_title_hide(LaunchSource::Lotd),
            UiAction::StartMatch => self.start_title_hide(LaunchSource::Match { mirror: false }),
            UiAction::StartMirrorMatch => {
                self.start_title_hide(LaunchSource::Match { mirror: true })
            }
            UiAction::OpenLevelmakerNotice => {
                self.push_notice("The Milkman Games Levelmaker online service is no longer active.\n\nCampaign and Match Modes remain playable offline.");
            }
            UiAction::OpenMmgNotice => {
                self.push_notice(
                    "Aqualux Deluxe\nCopyright (C) 2010 Milkman Games, LLC.\nAll Rights Reserved.\naqualux-rs reimplementation by ddavef",
                );
            }
            UiAction::OpenTrophyRoom => {
                self.modals.push(ModalState::TrophyRoom {
                    page: 0,
                    hover_idx: None,
                });
                let has_save = self
                    .save
                    .active_profile()
                    .is_some_and(|p| p.has_saved_game());
                if !has_save {
                    self.modals.push(ModalState::Question {
                        id: QuestionId::TrophyNeedSave,
                        message: "Start a new game from the main menu to start earning trophies!"
                            .to_string(),
                        yes_label: "Ok",
                        no_label: None,
                    });
                }
            }
            UiAction::OpenOptions => {
                let allow_diff = match self.screen {
                    Screen::Title => self
                        .save
                        .active_profile()
                        .is_some_and(|p| p.has_saved_game()),
                    Screen::Paused => self.match_sim.is_none() && !self.is_lotd_mode,
                    _ => false,
                };
                self.modals.push(ModalState::Options {
                    allow_difficulty: allow_diff,
                });
            }
            UiAction::OpenProfileModal => self.open_profile_modal(),
            UiAction::QuitPrompt => {
                self.modals.push(ModalState::Question {
                    id: QuestionId::ConfirmQuit,
                    message: "Really quit the game?".to_string(),
                    yes_label: "yes",
                    no_label: Some("no"),
                });
            }

            UiAction::PauseGame => {
                if let Some(s) = &mut self.campaign_sim {
                    s.is_paused = true;
                }
                if let Some(s) = &mut self.match_sim {
                    s.is_paused = true;
                }
                for l in &mut self.leaks {
                    l.paused = true;
                }
                self.bg_anim.darken(0.0);
                self.screen = Screen::Paused;
            }
            UiAction::ResumeGame => {
                self.bg_anim.lighten(0.0);
                for l in &mut self.leaks {
                    l.paused = false;
                }
                if let Some(s) = &mut self.campaign_sim {
                    s.is_paused = false;
                    self.screen = Screen::Playing;
                } else if let Some(s) = &mut self.match_sim {
                    s.is_paused = false;
                    self.screen = Screen::MatchPlaying;
                } else {
                    self.screen = Screen::Title;
                }
            }
            UiAction::FastForward => {
                self.dispatch_campaign_sim(|s| s.fast_forward());
            }
            UiAction::NextTutorial => {
                self.dispatch_campaign_sim(|s| s.next_tutorial());
            }
            UiAction::SkipTutorials => {
                self.dispatch_campaign_sim(|s| s.skip_tutorials());
            }
            UiAction::MainMenuPrompt => {
                let will_save = if self.is_lotd_mode {
                    ""
                } else {
                    " Your progress will be saved."
                };
                self.modals.push(ModalState::Question {
                    id: QuestionId::ConfirmMainMenuFromPause,
                    message: format!("Return to main menu? \n \n{}", will_save),
                    yes_label: "yes",
                    no_label: Some("no"),
                });
            }

            UiAction::NextLevel => self.start_summary_hide(SummaryHideAction::NextLevel),
            UiAction::RetryLevel => self.start_summary_hide(SummaryHideAction::RetryLevel),
            UiAction::MainMenuConfirmFromSummary => {
                let will_save = if self.is_lotd_mode {
                    ""
                } else {
                    " Your progress will be saved."
                };
                self.modals.push(ModalState::Question {
                    id: QuestionId::ConfirmMainMenuFromSummary,
                    message: format!("Return to main menu? \n \n{}", will_save),
                    yes_label: "yes",
                    no_label: Some("no"),
                });
            }
            UiAction::DirectMainMenu => {
                if let Some(ch) = &mut self.champion_state {
                    ch.start_hide(false);
                } else {
                    self.return_to_main_menu();
                }
            }
            UiAction::DirectQuit => {
                if let Some(ch) = &mut self.champion_state {
                    ch.start_hide(true);
                } else {
                    self.should_quit = true;
                }
            }

            UiAction::SetMusicVol(v) => {
                self.save.config.music_volume = v;
                self.audio.music_volume = v;
                self.audio.apply_volumes();
                self.save.save();
            }
            UiAction::SetSfxVol(v) => {
                self.save.config.sfx_volume = v;
                self.audio.sfx_volume = v;
                self.save.save();
            }
            UiAction::ToggleColorBlind => {
                self.save.config.color_blind = !self.save.config.color_blind;
                self.save.save();
            }
            UiAction::ToggleWindowed => {
                self.save.config.fullscreen = !self.save.config.fullscreen;
                self.sync_fullscreen_req = true;
                self.save.save();
            }
            UiAction::CycleFps => {
                let cur = self.save.config.target_fps;
                let next_idx = FPS_CYCLE
                    .iter()
                    .position(|&f| f == cur)
                    .map(|i| (i + 1) % FPS_CYCLE.len())
                    .unwrap_or(2);
                self.save.config.target_fps = FPS_CYCLE[next_idx];
                self.save.save();
            }
            UiAction::OpenOptionsDifficulty => {
                let cur = self
                    .save
                    .active_profile()
                    .map(|p| p.savable_difficulty)
                    .unwrap_or_default();
                self.modals.push(ModalState::Difficulty {
                    selected: cur,
                    ok_only: true,
                    trophy_level_idx: None,
                });
            }
            UiAction::CloseTopModal => {
                self.modals.pop();
            }

            UiAction::SelectDifficultyRadio(d) => {
                if let Some(ModalState::Difficulty { selected, .. }) = self.modals.last_mut() {
                    *selected = d;
                }
            }
            UiAction::ConfirmDifficultyModal => self.confirm_difficulty_modal(),

            UiAction::TrophyNextPage => {
                let total_slots = self.assets.campaign_levels.len() + 4;
                let pages = total_slots.div_ceil(26);
                if let Some(ModalState::TrophyRoom { page, .. }) = self.modals.last_mut() {
                    *page = if *page + 1 >= pages { 0 } else { *page + 1 };
                }
            }
            UiAction::TrophyPickLevel(idx) => {
                let cur = self
                    .save
                    .active_profile()
                    .map(|p| p.savable_difficulty)
                    .unwrap_or_default();
                self.modals.push(ModalState::Difficulty {
                    selected: cur,
                    ok_only: false,
                    trophy_level_idx: Some(idx),
                });
            }

            UiAction::ProfileSelectIdx(_)
            | UiAction::ProfileNew
            | UiAction::ProfileDelete
            | UiAction::ProfileAccept => self.handle_profile_action(action),
            UiAction::SubmitName => self.handle_submit_name(),

            UiAction::ToggleLevelOnlyCheckbox => {
                if let Some(ModalState::InGameDifficulty { level_only }) = self.modals.last_mut() {
                    *level_only = !*level_only;
                }
            }
            UiAction::InGameDiffYes => {
                if let Some(ModalState::InGameDifficulty { level_only }) = self.modals.pop() {
                    if let Some(prof) = self.save.active_profile_mut() {
                        let prev = prof.difficulty;
                        prof.difficulty = Difficulty::Easy;
                        if level_only {
                            self.temp_easy_revert = Some(prev);
                        } else {
                            prof.savable_difficulty = Difficulty::Easy;
                        }
                    }
                    self.save.save();
                    self.show_status_toast(
                        "TIP: You can change the difficulty later from the options menu.",
                    );
                    self.repeat_current_level();
                }
            }
            UiAction::InGameDiffNo => {
                self.modals.pop();
                self.show_status_toast(
                    "TIP: You can change the difficulty later from the options menu.",
                );
                self.repeat_current_level();
            }

            UiAction::QuestionYes => self.handle_question_yes(),
            UiAction::QuestionNo => {
                self.modals.pop();
            }
            UiAction::DismissAward => self.handle_dismiss_award(),
        }
    }

    pub(crate) fn start_title_hide(&mut self, source: LaunchSource) {
        self.title_hiding = Some((0.0, source));
        self.audio.play_sfx_delayed("SwapTwoSound", 0.4, 1.0);
    }

    fn dispatch_campaign_sim(
        &mut self,
        f: impl FnOnce(&mut crate::game::LevelSim) -> Vec<crate::game::SimEvent>,
    ) {
        let layout_opt = self.campaign_sim.as_ref().map(|sim| {
            BoardLayout::compute(sim.level.cols, sim.level.rows, hud::campaign_board_area())
        });
        let events = self.campaign_sim.as_mut().map(f).unwrap_or_default();
        if let Some(layout) = layout_opt {
            self.handle_sim_events(events, layout);
        }
    }

    fn push_notice(&mut self, message: &str) {
        self.modals.push(ModalState::Question {
            id: QuestionId::SimpleNotice,
            message: message.to_string(),
            yes_label: "Ok",
            no_label: None,
        });
    }

    fn confirm_difficulty_modal(&mut self) {
        let Some(ModalState::Difficulty {
            selected,
            ok_only,
            trophy_level_idx,
        }) = self.modals.pop()
        else {
            return;
        };
        if ok_only {
            self.temp_easy_revert = None;
            if let Some(prof) = self.save.active_profile_mut() {
                prof.difficulty = selected;
                prof.savable_difficulty = selected;
            }
            self.save.save();
        } else if let Some(lvl_idx) = trophy_level_idx {
            self.modals.clear();
            self.start_title_hide(LaunchSource::RetryTrophy {
                idx: lvl_idx,
                difficulty: selected,
            });
        }
    }
}
