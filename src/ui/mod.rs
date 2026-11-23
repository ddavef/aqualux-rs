// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

pub mod actions;
pub mod campaign_end;
pub mod champion;
pub mod custom_hub;
pub mod custom_hub_actions;
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
use crate::game::{Difficulty, LevelSim, MatchSim};
use crate::render::{
    BgAnimState, BoardLayout, FireworkBurst, GpuCore, LeakEmitter, Renderer, ScoreBlurbFx,
};
use crate::save::{LevelRecord, SaveData};
use champion::ChampionState;
use custom_hub::CustomHubState;
use rand::rngs::StdRng;
use std::path::Path;
use std::sync::Arc;
use summary::SummaryState;
use title::TitleSubmenu;
use widgets::{LaunchSource, ModalState, ScoreRoller};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Splash,
    Title,
    CustomHub,
    Playing,
    MatchPlaying,
    Paused,
    Summary,
    Champion,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DragSlider {
    Music,
    Sfx,
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
    pub(crate) title_submenu: TitleSubmenu,
    pub(crate) splash_age: f32,
    pub(crate) splash_music_started: bool,
    pub(crate) title_age: f32,
    pub(crate) title_submenu_age: f32,
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
    pub custom_packs: CustomPackStore,
    pub custom_hub: CustomHubState,
    pub fail_count: u32,
    pub is_retrying_level: bool,
    pub retry_backup: Option<LevelRecord>,
    pub temp_easy_revert: Option<Difficulty>,
    /// `(elapsed, start_delay, done_time, pop_sfx_count)`
    pub(crate) board_pop_elapsed: Option<(f32, f32, f32, usize)>,
    /// `(elapsed, delay, done_time, next_step)`
    pub(crate) board_hide: Option<(f32, f32, f32, HideAfterBoard)>,
    pub(crate) pause_fade: f32,
    pub(crate) tut_age: f32,
    pub(crate) last_tut_idx: usize,
    pub(crate) hud_score_roller: ScoreRoller,
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
        let custom_packs = CustomPackStore::load(custom_levels_dir);
        let renderer = Renderer::new(gpu, &assets)?;
        let audio = AudioEngine::new(
            assets.sounds.clone(),
            save.config.music_volume,
            save.config.sfx_volume,
        );
        let init_score = save.active_profile().map(|p| p.total_score()).unwrap_or(0);
        Ok(Self {
            assets,
            save,
            audio,
            renderer,
            bg_anim: BgAnimState::new("NightHills"),
            screen: Screen::Splash,
            modals: Vec::new(),
            title_submenu: TitleSubmenu::Main,
            splash_age: 0.0,
            splash_music_started: false,
            title_age: 0.0,
            title_submenu_age: 0.0,
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
            custom_packs,
            custom_hub: CustomHubState::default(),
            fail_count: 0,
            is_retrying_level: false,
            retry_backup: None,
            temp_easy_revert: None,
            board_pop_elapsed: None,
            board_hide: None,
            pause_fade: 1.0,
            tut_age: 0.0,
            last_tut_idx: 0,
            hud_score_roller: ScoreRoller::new(init_score),
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
        })
    }

    pub fn target_fps(&self) -> u32 {
        self.save.config.target_fps
    }

    pub(crate) fn is_board_busy(&self) -> bool {
        self.board_pop_elapsed.is_some() || self.board_hide.is_some()
    }

    pub fn update(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        self.time += dt;
        self.audio.update(dt);
        self.bg_anim.update(dt);

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

        let target_hud =
            self.display_base_score() + self.campaign_sim.as_ref().map(|s| s.score).unwrap_or(0);
        self.hud_score_roller.roll_to(target_hud, 1.0, 0.0);
        self.hud_score_roller.update(dt);

        if self.screen == Screen::Splash {
            self.splash_age += dt;
            if self.splash_age >= 0.4 && !self.splash_music_started {
                self.splash_music_started = true;
                self.audio.play_music("LittleRayMusic", true);
            }
            if self.splash_age >= 3.9 {
                self.screen = Screen::Title;
                self.bg_anim = BgAnimState::new("NightHills");
                self.title_age = 0.0;
                if !self.save.has_profile() {
                    self.modals.push(ModalState::EnterName {
                        prompt: "Welcome To Aqualux!\n\nPlease enter your name:".to_string(),
                        input: String::new(),
                        allow_cancel: false,
                        creating_for_profile_list: false,
                        pending_launch_after: false,
                    });
                }
            }
            return;
        }

        if self.screen == Screen::Title {
            self.title_age += dt;
            self.title_submenu_age += dt;
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
            self.pause_fade = (self.pause_fade - dt * 2.0).max(0.0);
        } else {
            self.pause_fade = 1.0;
        }

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
                }
                if let Some(ms) = &mut self.match_sim {
                    ms.start_level();
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
                        self.modals
                            .push(ModalState::InGameDifficulty { level_only: false });
                    }
                    HideAfterBoard::AdvanceMatch => self.advance_after_match(),
                }
            } else {
                self.board_hide = Some((elapsed, delay, done_t, next));
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
