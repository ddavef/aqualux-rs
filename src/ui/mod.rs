// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

pub mod actions;
pub mod campaign_end;
pub mod champion;
pub mod custom_hub;
pub mod custom_hub_actions;
pub mod editor;
pub mod editor_actions;
pub mod flows;
pub mod hud;
pub mod input;
pub mod modal_actions;
pub mod modals;
pub mod profiles;
pub mod render;
pub mod roller;
pub mod splash;
pub mod summary;
pub mod title;
pub mod title_data;
pub mod trophy_room;
pub mod widgets;

use crate::assets::AssetBundle;
use crate::audio::AudioEngine;
use crate::custom::CustomPackStore;
use crate::game::{Difficulty, LevelDef, LevelSim, MatchSim, MetricResult};
use crate::render::{
    BgAnimState, BoardLayout, FireworkBurst, GpuCore, LeakEmitter, Renderer, ScoreBlurbFx,
};
pub use crate::save::{EndlessConfig, EndlessRules};
use crate::save::{LevelRecord, SaveData};
use champion::ChampionState;
use custom_hub::CustomHubState;
use editor::EditorState;
use rand::rngs::StdRng;
use std::path::Path;
use std::sync::Arc;
use summary::SummaryState;
use title::{TitleMenuAnim, TitleProfileAnim, TitleSubmenu};
use widgets::{LaunchSource, ModalState, ScoreRoller};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Splash,
    Title,
    CustomHub,
    Editor,
    Playing,
    MatchPlaying,
    Paused,
    Summary,
    Champion,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum PlayMode {
    #[default]
    Campaign,
    Lotd,
    CustomPack {
        pack_id: String,
        level_idx: usize,
        session_score: u32,
        difficulty: Difficulty,
    },
    EditorTest {
        difficulty: Difficulty,
    },
    Endless {
        config: EndlessConfig,
        level_num: usize,
        pre_level_score: u32,
        session_score: u32,
        current_level: Box<LevelDef>,
        current_difficulty: Difficulty,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DragSlider {
    Music,
    Sfx,
    HdrPaperWhite,
    HdrPeak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HideAfterBoard {
    None,
    RepeatLevel,
    OfferEasyDifficulty,
    AdvanceMatch,
}

pub struct InfoBanner {
    pub title: String,
    pub description: String,
    pub elapsed: f32,
    pub delay: f32,
}

pub struct GameApp {
    pub assets: AssetBundle,
    pub save: SaveData,
    pub audio: AudioEngine,
    pub renderer: Renderer,
    pub bg_anim: BgAnimState,
    pub screen: Screen,
    pub(crate) modals: Vec<ModalState>,
    pub(crate) modal_ages: Vec<f32>,
    pub(crate) trophy_hover_age: f32,
    pub hdr_available: bool,
    pub hdr_manual_calibration: bool,
    pub(crate) options_pending_fullscreen: Option<bool>,
    pub(crate) options_pending_hdr: Option<bool>,
    pub(crate) options_board_color_blind: Option<bool>,
    pub(crate) title_submenu: TitleSubmenu,
    pub(crate) splash_age: f32,
    pub(crate) splash_overlay_age: Option<f32>,
    pub(crate) splash_music_started: bool,
    pub(crate) title_age: f32,
    pub(crate) title_menu_anim: TitleMenuAnim,
    pub(crate) title_wait_profile_latched: bool,
    pub(crate) title_show_complete_fired: bool,
    pub(crate) title_profile_anim: Option<TitleProfileAnim>,
    pub(crate) title_hiding: Option<(f32, LaunchSource)>,
    pub campaign_sim: Option<LevelSim>,
    pub match_sim: Option<MatchSim>,
    pub summary_state: Option<SummaryState>,
    pub champion_state: Option<ChampionState>,
    pub info_banner: Option<InfoBanner>,
    pub status_toast: Option<(String, f32)>,
    pub current_level_idx: usize,
    pub play_mode: PlayMode,
    pub lotd_index: usize,
    pub lotd_score: u32,
    pub(crate) lotd_level_scores: Vec<u32>,
    pub custom_packs: CustomPackStore,
    pub custom_hub: CustomHubState,
    pub editor: Option<EditorState>,
    pub endless_config: EndlessConfig,
    pub fail_count: u32,
    pub is_index_savable: bool,
    pub retry_backup: Option<LevelRecord>,
    pub(crate) pending_summary: Option<(MetricResult, u32, u32)>,
    pub temp_easy_revert: Option<Difficulty>,
    /// `(elapsed, start_delay, done_time, pop_sfx_count)`
    pub(crate) board_pop_elapsed: Option<(f32, f32, f32, usize)>,
    /// `(elapsed, delay, done_time, next_step)`
    pub(crate) board_hide: Option<(f32, f32, f32, HideAfterBoard)>,
    pub(crate) board_age: f32,
    pub(crate) pause_elapsed: f32,
    pub(crate) pause_fade: f32,
    pub(crate) tut_age: f32,
    pub(crate) last_tut_idx: usize,
    pub(crate) ff_hint_age: Option<f32>,
    pub(crate) hud_anim: hud::HudAnimState,
    pub(crate) hud_tutorial_active: bool,
    pub(crate) hud_score_roller: ScoreRoller,
    pub(crate) hud_moves_roller: ScoreRoller,
    pub(crate) hud_time_roller: ScoreRoller,
    pub mouse_pos: (f32, f32),
    pub cursor_in_window: bool,
    pub drag_start_tile: Option<(usize, usize)>,
    pub(crate) dragging_slider: Option<DragSlider>,
    pub(crate) hovered_button_id: Option<&'static str>,
    pub(crate) hovered_tile: Option<(usize, usize)>,
    pub time: f32,
    pub blurbs: Vec<ScoreBlurbFx>,
    pub leaks: Vec<LeakEmitter>,
    pub fireworks: Vec<FireworkBurst>,
    pub rng: StdRng,
    pub should_quit: bool,
    pub sync_fullscreen_req: bool,
    pub sync_hdr_req: bool,
}

impl GameApp {
    pub fn new(
        assets: AssetBundle,
        fps_override: Option<u32>,
        gpu: Arc<GpuCore>,
    ) -> Result<Self, String> {
        Self::new_with_custom_dir(assets, fps_override, None, gpu)
    }

    pub fn new_with_custom_dir(
        assets: AssetBundle,
        fps_override: Option<u32>,
        custom_levels_dir: Option<&Path>,
        gpu: Arc<GpuCore>,
    ) -> Result<Self, String> {
        let mut save = SaveData::load();
        if let Some(fps) = fps_override {
            save.config.target_fps = fps;
        }
        let hdr_caps = gpu.hdr_caps();
        let hdr_available = hdr_caps.available;
        let hdr_manual_calibration = hdr_caps.needs_manual_calibration;
        let custom_packs = CustomPackStore::load(custom_levels_dir);
        let renderer = Renderer::new(gpu, &assets)?;
        let audio = AudioEngine::new(
            assets.sounds.clone(),
            save.config.music_volume,
            save.config.sfx_volume,
        );
        let init_score = save.active_profile().map(|p| p.total_score()).unwrap_or(0);
        let init_endless = save
            .active_profile()
            .map(|p| p.endless_config.sanitized())
            .unwrap_or_default();
        Ok(Self {
            assets,
            save,
            audio,
            renderer,
            bg_anim: BgAnimState::new("NightHills"),
            screen: Screen::Splash,
            modals: Vec::new(),
            modal_ages: Vec::new(),
            trophy_hover_age: 0.0,
            hdr_available,
            hdr_manual_calibration,
            options_pending_fullscreen: None,
            options_pending_hdr: None,
            options_board_color_blind: None,
            title_submenu: TitleSubmenu::Main,
            splash_age: 0.0,
            splash_overlay_age: None,
            splash_music_started: false,
            title_age: 0.0,
            title_menu_anim: TitleMenuAnim::InitialSlide,
            title_wait_profile_latched: false,
            title_show_complete_fired: false,
            title_profile_anim: None,
            title_hiding: None,
            campaign_sim: None,
            match_sim: None,
            summary_state: None,
            champion_state: None,
            info_banner: None,
            status_toast: None,
            current_level_idx: 0,
            play_mode: PlayMode::Campaign,
            lotd_index: 0,
            lotd_score: 0,
            lotd_level_scores: Vec::new(),
            custom_packs,
            custom_hub: CustomHubState::default(),
            editor: None,
            endless_config: init_endless,
            fail_count: 0,
            is_index_savable: true,
            retry_backup: None,
            pending_summary: None,
            temp_easy_revert: None,
            board_pop_elapsed: None,
            board_hide: None,
            board_age: 0.0,
            pause_elapsed: 0.0,
            pause_fade: 1.0,
            tut_age: 0.0,
            last_tut_idx: 0,
            ff_hint_age: None,
            hud_anim: hud::HudAnimState::default(),
            hud_tutorial_active: false,
            hud_score_roller: ScoreRoller::new(init_score),
            hud_moves_roller: ScoreRoller::new(0),
            hud_time_roller: ScoreRoller::new(0),
            mouse_pos: (400.0, 300.0),
            cursor_in_window: true,
            drag_start_tile: None,
            dragging_slider: None,
            hovered_button_id: None,
            hovered_tile: None,
            time: 0.0,
            blurbs: Vec::new(),
            leaks: Vec::new(),
            fireworks: Vec::new(),
            rng: rand::make_rng(),
            should_quit: false,
            sync_fullscreen_req: false,
            sync_hdr_req: false,
        })
    }

    #[cfg(test)]
    pub fn set_hdr_available(&mut self, available: bool) {
        self.set_hdr_display_state(available, self.hdr_manual_calibration);
    }

    pub fn set_hdr_display_state(&mut self, available: bool, manual_calibration: bool) {
        self.hdr_available = available;
        self.hdr_manual_calibration = manual_calibration;
        if !available && let Some(pending) = &mut self.options_pending_hdr {
            *pending = false;
        }
    }

    pub fn target_fps(&self) -> u32 {
        self.save.config.target_fps
    }

    pub fn set_window_focused(&mut self, focused: bool) {
        self.audio.set_window_focused(focused);
    }

    pub(crate) fn push_modal(&mut self, modal: ModalState) {
        self.modal_ages.resize(self.modals.len(), 1.0);
        self.modals.push(modal);
        self.modal_ages.push(0.0);
    }

    pub(crate) fn pop_modal(&mut self) -> Option<ModalState> {
        let m = self.modals.pop();
        self.modal_ages.truncate(self.modals.len());
        m
    }

    pub(crate) fn clear_modals(&mut self) {
        self.modals.clear();
        self.modal_ages.clear();
    }

    pub(crate) fn commit_options_done(&mut self) {
        self.pop_modal();
        if let Some(want_full) = self.options_pending_fullscreen.take()
            && want_full != self.save.config.fullscreen
        {
            self.save.config.fullscreen = want_full;
            self.sync_fullscreen_req = true;
        }
        if let Some(want_hdr) = self.options_pending_hdr.take()
            && self.hdr_available
            && want_hdr != self.save.config.hdr
        {
            self.save.config.hdr = want_hdr;
            self.sync_hdr_req = true;
        }
        self.options_board_color_blind = None;
        self.save.save();
    }

    pub(crate) fn active_endless_config(&self) -> EndlessConfig {
        self.save
            .active_profile()
            .map(|p| p.endless_config.sanitized())
            .unwrap_or_else(|| self.endless_config.sanitized())
    }

    pub(crate) fn sync_endless_config_from_profile(&mut self) {
        self.endless_config = self.active_endless_config();
    }

    pub(crate) fn is_board_busy(&self) -> bool {
        self.board_pop_elapsed.is_some() || self.board_hide.is_some()
    }

    pub(crate) fn can_pause(&self) -> bool {
        if self.is_board_busy() || !self.modals.is_empty() {
            return false;
        }
        match self.screen {
            Screen::Playing => self
                .campaign_sim
                .as_ref()
                .is_some_and(|s| s.active_tutorial().is_none() && s.is_started && !s.finished),
            Screen::MatchPlaying => self
                .match_sim
                .as_ref()
                .is_some_and(|m| m.is_started && !m.finished),
            _ => false,
        }
    }

    pub(crate) fn enter_title_screen(&mut self) {
        self.screen = Screen::Title;
        self.title_submenu = TitleSubmenu::Main;
        self.title_age = 0.0;
        self.title_menu_anim = TitleMenuAnim::InitialSlide;
        self.title_hiding = None;
        self.title_show_complete_fired = false;
        if self.save.has_profile() {
            self.title_wait_profile_latched = false;
            self.title_profile_anim = Some(TitleProfileAnim::on_title_enter());
        } else {
            self.title_wait_profile_latched = true;
            self.title_profile_anim = None;
        }
    }

    pub fn update(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        self.time += dt;
        self.audio.update(dt);
        self.bg_anim.update(dt);
        if let Some(sa) = &mut self.splash_overlay_age {
            *sa += dt;
            if *sa >= splash::SPLASH_END {
                self.splash_overlay_age = None;
            }
        }
        if self.screen != Screen::Champion
            && let Some(ch) = &mut self.champion_state
            && ch.update(dt).is_some()
        {
            self.champion_state = None;
        }

        if let Some((text, mut age)) = self.status_toast.take() {
            age += dt;
            if age <= 4.5 {
                self.status_toast = Some((text, age));
            }
        }
        if let Some(banner) = &mut self.info_banner {
            banner.elapsed += dt;
            if banner.elapsed - banner.delay > 4.8 {
                self.info_banner = None;
            }
        }

        for b in &mut self.blurbs {
            b.age += dt;
        }
        self.blurbs.retain(|b| !b.is_done());
        for l in &mut self.leaks {
            l.update(dt);
        }
        for f in &mut self.fireworks {
            f.update(dt);
        }
        self.fireworks.retain(|f| f.is_alive());

        let sim_live_score = self
            .campaign_sim
            .as_ref()
            .filter(|s| !s.finished)
            .map(|s| s.score)
            .unwrap_or(0);
        let target_hud = self.display_base_score() + sim_live_score;
        self.hud_score_roller.roll_to(target_hud, 1.0, 0.0);
        self.hud_score_roller.update(dt);
        if let Some(sim) = &self.campaign_sim {
            self.hud_moves_roller
                .roll_to(sim.moves, if sim.moves == 0 { 1.0 } else { 0.0 }, 0.0);
            let time_ms = sim.elapsed_ms as u32;
            self.hud_time_roller
                .roll_to(time_ms, if time_ms == 0 { 1.0 } else { 0.0 }, 0.0);
        }
        self.hud_moves_roller.update(dt);
        self.hud_time_roller.update(dt);

        self.modal_ages.truncate(self.modals.len());
        if self.modal_ages.len() < self.modals.len() {
            self.modal_ages.resize(self.modals.len(), 1.0);
        }
        for age in &mut self.modal_ages {
            *age += dt;
        }
        if matches!(
            self.modals.last(),
            Some(ModalState::TrophyRoom {
                hover_idx: Some(_),
                ..
            })
        ) {
            self.trophy_hover_age += dt;
        } else {
            self.trophy_hover_age = 0.0;
        }
        self.hud_anim.update(dt);

        if self.screen == Screen::Splash {
            self.splash_age += dt;
            if self.splash_age >= 0.4 && !self.splash_music_started {
                self.splash_music_started = true;
                self.audio.play_music("LittleRayMusic", true);
            }
            if self.splash_age >= 3.9 {
                self.splash_overlay_age =
                    (self.splash_age < splash::SPLASH_END).then_some(self.splash_age);
                self.bg_anim = BgAnimState::new("NightHills");
                self.enter_title_screen();
            }
            return;
        }

        if self.screen == Screen::Title {
            self.title_age += dt;
            if let TitleMenuAnim::FadeIn { elapsed } = &mut self.title_menu_anim {
                *elapsed += dt;
            }
            if let Some(pa) = &mut self.title_profile_anim {
                pa.elapsed += dt;
            }
            if !self.title_show_complete_fired && self.title_age >= 6.0 {
                self.title_show_complete_fired = true;
                if self.title_wait_profile_latched && !self.save.has_profile() {
                    self.push_modal(ModalState::EnterName {
                        prompt: "Welcome To Aqualux!\n\nPlease enter your name:".to_string(),
                        input: String::new(),
                        allow_cancel: false,
                        creating_for_profile_list: false,
                        pending_launch_after: false,
                    });
                }
            }
            if let Some((elapsed, src)) = self.title_hiding.take() {
                let next_e = elapsed + dt;
                if next_e >= 1.5 {
                    self.execute_launch(src);
                } else {
                    self.title_hiding = Some((next_e, src));
                }
            }
            return;
        }

        if self.screen == Screen::Paused {
            self.pause_elapsed = (self.pause_elapsed + dt).min(1.0);
            self.pause_fade =
                1.0 - crate::render::easing::quad_out((self.pause_elapsed / 0.5).clamp(0.0, 1.0));
        } else {
            self.pause_elapsed = 0.0;
            self.pause_fade = 1.0;
            if self.campaign_sim.is_some() || self.match_sim.is_some() {
                self.board_age += dt;
            }
        }

        if self.screen != Screen::Paused {
            if let Some((mut elapsed, start_d, done_t, mut sfx_done)) = self.board_pop_elapsed {
                elapsed += dt;
                let total_tiles = self
                    .campaign_sim
                    .as_ref()
                    .map(|s| s.level.rows * s.level.cols)
                    .or_else(|| self.match_sim.as_ref().map(|m| m.level.rows * m.level.cols))
                    .unwrap_or(0);
                while sfx_done < total_tiles && elapsed >= start_d + (sfx_done as f32) * 0.1 {
                    self.audio.play_sfx("PopTwoSound");
                    sfx_done += 1;
                }
                if elapsed >= done_t {
                    self.board_pop_elapsed = None;
                    if let Some(sim) = &mut self.campaign_sim {
                        let evs = sim.start_level();
                        let l = BoardLayout::compute(
                            sim.level.cols,
                            sim.level.rows,
                            hud::campaign_board_area(),
                        );
                        self.handle_sim_events(evs, l);
                        if self
                            .campaign_sim
                            .as_ref()
                            .is_some_and(|s| s.active_tutorial().is_some())
                        {
                            self.hud_tutorial_active = true;
                            self.hud_anim.show_skip_tutorial();
                            self.hud_anim.hide_action_buttons();
                        } else {
                            self.hud_tutorial_active = false;
                            self.hud_anim.show_action_buttons();
                        }
                    }
                    if let Some(ms) = &mut self.match_sim {
                        ms.start_level();
                        self.hud_anim.show_action_buttons();
                    }
                } else {
                    self.board_pop_elapsed = Some((elapsed, start_d, done_t, sfx_done));
                }
            }

            if let Some((mut elapsed, delay, done_t, next)) = self.board_hide {
                elapsed += dt;
                if elapsed >= done_t {
                    self.board_hide = None;
                    match next {
                        HideAfterBoard::None => {}
                        HideAfterBoard::RepeatLevel => self.repeat_current_level(),
                        HideAfterBoard::OfferEasyDifficulty => {
                            self.push_modal(ModalState::InGameDifficulty { level_only: false });
                        }
                        HideAfterBoard::AdvanceMatch => self.advance_after_match(),
                    }
                } else {
                    self.board_hide = Some((elapsed, delay, done_t, next));
                }
            }
        }

        if self.screen == Screen::Playing && self.modals.is_empty() {
            let layout_opt = self.campaign_sim.as_ref().map(|sim| {
                BoardLayout::compute(sim.level.cols, sim.level.rows, hud::campaign_board_area())
            });
            let cur_tut = self
                .campaign_sim
                .as_ref()
                .map(|s| s.tutorial_idx)
                .unwrap_or(0);
            if cur_tut != self.last_tut_idx {
                self.last_tut_idx = cur_tut;
                self.tut_age = 0.0;
            } else if self
                .campaign_sim
                .as_ref()
                .is_some_and(|s| s.active_tutorial().is_some())
            {
                self.tut_age += dt;
            }
            let events = self
                .campaign_sim
                .as_mut()
                .map(|s| s.update(dt))
                .unwrap_or_default();
            if let Some(layout) = layout_opt {
                self.handle_sim_events(events, layout);
            }
            let now_tut = self
                .campaign_sim
                .as_ref()
                .is_some_and(|s| s.active_tutorial().is_some());
            if now_tut && !self.hud_tutorial_active {
                self.hud_tutorial_active = true;
                self.hud_anim.show_skip_tutorial();
                self.hud_anim.hide_action_buttons();
            } else if !now_tut && self.hud_tutorial_active {
                self.hud_tutorial_active = false;
                self.hud_anim.hide_skip_tutorial();
                self.hud_anim.show_action_buttons();
            }
            let show_ff_hint = self.play_mode == PlayMode::Campaign
                && self.campaign_sim.as_ref().is_some_and(|s| {
                    s.is_started
                        && !s.finished
                        && !s.is_accelerated
                        && s.active_tutorial().is_none()
                        && s.path_complete
                        && s.level.index <= 4
                });
            if show_ff_hint {
                if let Some(age) = &mut self.ff_hint_age {
                    *age += dt;
                } else {
                    self.ff_hint_age = Some(0.0);
                    self.audio.play_sfx_delayed("PopTwoSound", 0.2, 1.0);
                }
            } else {
                self.ff_hint_age = None;
            }
        } else if self.screen == Screen::MatchPlaying && self.modals.is_empty() {
            let layout_opt = self.match_sim.as_ref().map(|sim| {
                BoardLayout::compute(sim.level.cols, sim.level.rows, hud::match_play_area())
            });
            let events = self
                .match_sim
                .as_mut()
                .map(|s| s.update(dt))
                .unwrap_or_default();
            if let Some(layout) = layout_opt {
                self.handle_sim_events(events, layout);
            }
        } else {
            self.tick_end_screens(dt);
        }
    }
}
