// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::Difficulty;
use crate::render::BoardLayout;
use crate::render::hdr::{HDR_PAPER_WHITE_MAX, HDR_PAPER_WHITE_MIN, HDR_PEAK_MAX, HDR_PEAK_MIN};
use crate::ui::hud;
use crate::ui::summary::SummaryHideAction;
use crate::ui::title::{TitleMenuAnim, TitleSubmenu};
use crate::ui::widgets::{LaunchSource, ModalState, QuestionId, UiAction};
use crate::ui::{GameApp, PlayMode, Screen};

const FPS_CYCLE: &[u32] = &[30, 60, 120, 144, 240, 0];

impl GameApp {
    pub(crate) fn trigger_action(&mut self, action: UiAction) {
        let silent = matches!(
            action,
            UiAction::SetMusicVol(_)
                | UiAction::SetSfxVol(_)
                | UiAction::ProfileSelectIdx(_)
                | UiAction::SelectDifficultyRadio(_)
                | UiAction::ToggleLevelOnlyCheckbox
                | UiAction::ToggleColorBlind
                | UiAction::ToggleWindowed
                | UiAction::ToggleHdr
                | UiAction::SetHdrPaperWhite(_)
                | UiAction::SetHdrPeak(_)
                | UiAction::EndlessToggleDifficulty(_)
                | UiAction::NextTutorial
                | UiAction::PauseGame
                | UiAction::FastForward
                | UiAction::SkipTutorials
                | UiAction::OpenMmgNotice
                | UiAction::TrophyPickLevel(_)
                | UiAction::ProfileNew
                | UiAction::ProfileDelete
                | UiAction::SubmitName
        ) || (matches!(action, UiAction::CloseTopModal)
            && matches!(self.modals.last(), Some(ModalState::EnterName { .. })));
        if !silent {
            self.audio.play_sfx("ClickOneSound");
        }

        match action {
            UiAction::OpenPlayMenu => {
                self.title_submenu = TitleSubmenu::PlayChoices;
                self.title_menu_anim = TitleMenuAnim::FadeIn { elapsed: 0.0 };
            }
            UiAction::BackToMainMenu => {
                self.title_submenu = TitleSubmenu::Main;
                self.title_menu_anim = TitleMenuAnim::FadeIn { elapsed: 0.0 };
            }
            UiAction::BackToPlayMenu => {
                self.title_submenu = TitleSubmenu::PlayChoices;
                self.title_menu_anim = TitleMenuAnim::FadeIn { elapsed: 0.0 };
            }
            UiAction::StartOverPrompt => {
                let has_save = self
                    .save
                    .active_profile()
                    .is_some_and(|p| p.has_saved_game());
                if has_save {
                    self.push_modal(ModalState::Question {
                        id: QuestionId::ConfirmNewGame,
                        message: "Really start a new game? \n \n  Saved progress will be lost."
                            .to_string(),
                        yes_label: "yes",
                        no_label: Some("no"),
                    });
                } else {
                    self.title_submenu = TitleSubmenu::DifficultyChoices;
                    self.title_menu_anim = TitleMenuAnim::FadeIn { elapsed: 0.0 };
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
            UiAction::OpenEndlessSetup => {
                let cfg = self.active_endless_config();
                self.endless_config = cfg;
                self.push_modal(ModalState::EndlessSetup(cfg));
            }
            UiAction::EndlessToggleDifficulty(d) => {
                if let Some(ModalState::EndlessSetup(cfg)) = self.modals.last_mut() {
                    cfg.toggle(d);
                }
            }
            UiAction::EndlessAdjustMaxCols(delta) => {
                if let Some(ModalState::EndlessSetup(cfg)) = self.modals.last_mut() {
                    cfg.adjust_max_cols(delta);
                }
            }
            UiAction::EndlessAdjustMaxRows(delta) => {
                if let Some(ModalState::EndlessSetup(cfg)) = self.modals.last_mut() {
                    cfg.adjust_max_rows(delta);
                }
            }
            UiAction::EndlessCycleRules => {
                if let Some(ModalState::EndlessSetup(cfg)) = self.modals.last_mut() {
                    cfg.rules = cfg.rules.next();
                }
            }
            UiAction::StartEndlessRun => {
                if let Some(&ModalState::EndlessSetup(raw_cfg)) = self.modals.last()
                    && raw_cfg.any_enabled()
                {
                    let cfg = raw_cfg.sanitized();
                    self.pop_modal();
                    self.endless_config = cfg;
                    if let Some(prof) = self.save.active_profile_mut() {
                        prof.endless_config = cfg;
                        self.save.save();
                    }
                    if self.screen == Screen::Title {
                        self.start_title_hide(LaunchSource::Endless(cfg));
                    } else {
                        self.execute_launch(LaunchSource::Endless(cfg));
                    }
                }
            }
            UiAction::OpenCustomHub { play_focused } => {
                if self.screen == Screen::Title {
                    self.start_title_hide(LaunchSource::CustomHub { play_focused });
                } else {
                    self.open_custom_hub(play_focused);
                }
            }
            UiAction::SubmitTextPrompt => self.handle_submit_text_prompt(),
            UiAction::HubBackToTitle
            | UiAction::HubOpenFolder
            | UiAction::HubReloadPacks
            | UiAction::HubSelectPack(_)
            | UiAction::HubPackPrevPage
            | UiAction::HubPackNextPage
            | UiAction::HubNewPack
            | UiAction::HubRenamePack
            | UiAction::HubDeletePack
            | UiAction::HubCloneEditable
            | UiAction::HubCycleDifficulty
            | UiAction::HubPlayPack
            | UiAction::HubSelectLevel(_)
            | UiAction::HubLevelPrevPage
            | UiAction::HubLevelNextPage
            | UiAction::HubPlayLevel
            | UiAction::HubEditLevel
            | UiAction::HubNewLevel
            | UiAction::HubGenerateLevel
            | UiAction::HubDuplicateLevel
            | UiAction::HubMoveLevelUp
            | UiAction::HubMoveLevelDown
            | UiAction::HubDeleteLevel => self.handle_custom_hub_action(action),
            UiAction::EditorSelectTool(_)
            | UiAction::EditorRotateBrushCw
            | UiAction::EditorSetBehavior(_)
            | UiAction::EditorToggleFreeze
            | UiAction::EditorToggleGate
            | UiAction::EditorToggleMagic
            | UiAction::EditorSetPortalColor(_)
            | UiAction::EditorResizeCols(_)
            | UiAction::EditorResizeRows(_)
            | UiAction::EditorUndo
            | UiAction::EditorRedo
            | UiAction::EditorClearBoard
            | UiAction::EditorAutoPar
            | UiAction::EditorToggleSolutionPath
            | UiAction::EditorOpenProps
            | UiAction::EditorVerify
            | UiAction::EditorTest
            | UiAction::EditorSave
            | UiAction::EditorBack
            | UiAction::PropsEditTitle
            | UiAction::PropsEditDesc
            | UiAction::PropsCycleBg
            | UiAction::PropsCycleLiquid
            | UiAction::PropsAdjustDelay(_)
            | UiAction::PropsAdjustSpeed(_)
            | UiAction::PropsCyclePipeColor(_)
            | UiAction::PropsAutoPar
            | UiAction::PropsAdjustParMoves(_)
            | UiAction::PropsAdjustParPipes(_)
            | UiAction::PropsAdjustParTimeSec(_)
            | UiAction::PropsCycleTestDiff
            | UiAction::PropsApply => self.handle_editor_action(action),
            UiAction::OpenMmgNotice => {
                self.push_notice(
                    "Aqualux Deluxe\nCopyright (C) 2010 Milkman Games, LLC.\nAll Rights Reserved.\naqualux-rs remaster by ddavef",
                );
            }
            UiAction::OpenTrophyRoom => {
                self.trophy_hover_age = 0.0;
                self.push_modal(ModalState::TrophyRoom {
                    page: 0,
                    hover_idx: None,
                });
                let has_save = self
                    .save
                    .active_profile()
                    .is_some_and(|p| p.has_saved_game());
                if !has_save {
                    self.push_modal(ModalState::Question {
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
                    Screen::Paused => {
                        self.match_sim.is_none() && self.play_mode == PlayMode::Campaign
                    }
                    _ => false,
                };
                self.options_pending_fullscreen = Some(self.save.config.fullscreen);
                self.options_pending_hdr = Some(self.save.config.hdr && self.hdr_available);
                self.options_board_color_blind = Some(self.save.config.color_blind);
                self.push_modal(ModalState::Options {
                    allow_difficulty: allow_diff,
                });
            }
            UiAction::OpenProfileModal => {
                self.open_profile_modal();
                if self.screen == Screen::Title && self.title_submenu != TitleSubmenu::Main {
                    self.title_submenu = TitleSubmenu::Main;
                    self.title_menu_anim = TitleMenuAnim::FadeIn { elapsed: 0.0 };
                }
            }
            UiAction::QuitPrompt => {
                self.push_modal(ModalState::Question {
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
                self.hud_anim.hide_overlay();
                self.bg_anim.darken(0.0);
                self.screen = Screen::Paused;
            }
            UiAction::ResumeGame => {
                self.bg_anim.lighten(0.0);
                for l in &mut self.leaks {
                    l.paused = false;
                }
                self.hud_anim.show_overlay();
                if !self.is_board_busy()
                    && !self.hud_tutorial_active
                    && !self.campaign_sim.as_ref().is_some_and(|s| s.is_accelerated)
                {
                    self.hud_anim.show_action_buttons();
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
                self.hud_anim.hide_action_buttons();
                self.dispatch_campaign_sim(|s| s.fast_forward());
            }
            UiAction::NextTutorial => {
                self.dispatch_campaign_sim(|s| s.next_tutorial());
            }
            UiAction::SkipTutorials => {
                self.dispatch_campaign_sim(|s| s.skip_tutorials());
                if self.hud_tutorial_active
                    && !self
                        .campaign_sim
                        .as_ref()
                        .is_some_and(|s| s.active_tutorial().is_some())
                {
                    self.hud_tutorial_active = false;
                    self.hud_anim.hide_skip_tutorial();
                    self.hud_anim.show_action_buttons();
                }
            }
            UiAction::MainMenuPrompt => {
                let msg = match self.play_mode {
                    PlayMode::Campaign => {
                        "Return to main menu? \n \n Your progress will be saved.".to_string()
                    }
                    PlayMode::CustomPack { .. } => "Return to custom packs?".to_string(),
                    PlayMode::EditorTest { .. } => "Return to the level editor?".to_string(),
                    PlayMode::Lotd | PlayMode::Endless { .. } => "Return to main menu?".to_string(),
                };
                self.push_modal(ModalState::Question {
                    id: QuestionId::ConfirmMainMenuFromPause,
                    message: msg,
                    yes_label: "yes",
                    no_label: Some("no"),
                });
            }

            UiAction::NextLevel => self.start_summary_hide(SummaryHideAction::NextLevel),
            UiAction::RetryLevel => self.start_summary_hide(SummaryHideAction::RetryLevel),
            UiAction::MainMenuConfirmFromSummary => {
                let msg = match self.play_mode {
                    PlayMode::Campaign => {
                        "Return to main menu? \n \n Your progress will be saved.".to_string()
                    }
                    PlayMode::CustomPack { .. } => "Return to custom packs?".to_string(),
                    PlayMode::EditorTest { .. } => "Return to the level editor?".to_string(),
                    PlayMode::Lotd | PlayMode::Endless { .. } => "Return to main menu?".to_string(),
                };
                self.push_modal(ModalState::Question {
                    id: QuestionId::ConfirmMainMenuFromSummary,
                    message: msg,
                    yes_label: "yes",
                    no_label: Some("no"),
                });
            }
            UiAction::DirectMainMenu => {
                if let Some(mut ch) = self.champion_state.take() {
                    ch.start_hide(false);
                    self.return_to_main_menu();
                    self.champion_state = Some(ch);
                } else {
                    self.return_to_main_menu();
                }
            }
            UiAction::DirectQuit => {
                if let Some(ch) = &mut self.champion_state {
                    self.audio.stop_music();
                    ch.start_hide(true);
                } else {
                    self.should_quit = true;
                }
            }

            UiAction::SetMusicVol(v) => {
                self.save.config.music_volume = v;
                self.audio.music_volume = v;
                self.audio.apply_volumes();
            }
            UiAction::SetSfxVol(v) => {
                self.save.config.sfx_volume = v;
                self.audio.sfx_volume = v;
            }
            UiAction::ToggleColorBlind => {
                self.save.config.color_blind = !self.save.config.color_blind;
            }
            UiAction::ToggleWindowed => {
                if let Some(f) = &mut self.options_pending_fullscreen {
                    *f = !*f;
                } else {
                    self.save.config.fullscreen = !self.save.config.fullscreen;
                }
            }
            UiAction::ToggleHdr => {
                if self.hdr_available {
                    if let Some(h) = &mut self.options_pending_hdr {
                        *h = !*h;
                    } else {
                        self.save.config.hdr = !self.save.config.hdr;
                        self.sync_hdr_req = true;
                    }
                }
            }
            UiAction::SetHdrPaperWhite(v) => {
                let nits = (HDR_PAPER_WHITE_MIN
                    + v.clamp(0.0, 1.0) * (HDR_PAPER_WHITE_MAX - HDR_PAPER_WHITE_MIN))
                    .round();
                self.save.config.hdr_paper_white_nits = nits;
            }
            UiAction::SetHdrPeak(v) => {
                let nits =
                    (HDR_PEAK_MIN + v.clamp(0.0, 1.0) * (HDR_PEAK_MAX - HDR_PEAK_MIN)).round();
                self.save.config.hdr_peak_nits = nits;
            }
            UiAction::CycleFps => {
                let cur = self.save.config.target_fps;
                let next_idx = FPS_CYCLE
                    .iter()
                    .position(|&f| f == cur)
                    .map(|i| (i + 1) % FPS_CYCLE.len())
                    .unwrap_or(2);
                self.save.config.target_fps = FPS_CYCLE[next_idx];
            }
            UiAction::OpenOptionsDifficulty => {
                let cur = self
                    .save
                    .active_profile()
                    .map(|p| p.savable_difficulty)
                    .unwrap_or_default();
                self.push_modal(ModalState::Difficulty {
                    selected: cur,
                    ok_only: true,
                    trophy_level_idx: None,
                });
            }
            UiAction::OptionsDone => self.commit_options_done(),
            UiAction::CloseTopModal => {
                self.pop_modal();
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
                if let Some(ModalState::TrophyRoom { page, hover_idx }) = self.modals.last_mut() {
                    *page = if *page + 1 >= pages { 0 } else { *page + 1 };
                    *hover_idx = None;
                }
                self.trophy_hover_age = 0.0;
            }
            UiAction::TrophyPickLevel(idx) => {
                let cur = self
                    .save
                    .active_profile()
                    .map(|p| p.savable_difficulty)
                    .unwrap_or_default();
                self.push_modal(ModalState::Difficulty {
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
                if let Some(ModalState::InGameDifficulty { level_only }) = self.pop_modal() {
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
                self.pop_modal();
                self.show_status_toast(
                    "TIP: You can change the difficulty later from the options menu.",
                );
                self.repeat_current_level();
            }

            UiAction::QuestionYes => self.handle_question_yes(),
            UiAction::QuestionNo => {
                self.pop_modal();
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
        self.push_modal(ModalState::Question {
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
        }) = self.pop_modal()
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
            self.clear_modals();
            self.start_title_hide(LaunchSource::RetryTrophy {
                idx: lvl_idx,
                difficulty: selected,
            });
        }
    }
}
