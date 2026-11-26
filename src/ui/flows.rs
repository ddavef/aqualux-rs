// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::audio::music_for_background;
use crate::game::generator::generate_custom_level_with_max_size;
use crate::game::{AwardKind, Difficulty, LevelDef, LevelSim, MatchSim, SimEvent};
use crate::render::{BoardLayout, LeakEmitter, ScoreBlurbFx};
use crate::ui::widgets::{LaunchSource, ModalState, ScoreRoller};
use crate::ui::{EndlessConfig, EndlessRules, GameApp, InfoBanner, PlayMode, Screen};

impl GameApp {
    pub(crate) fn execute_launch(&mut self, source: LaunchSource) {
        self.fail_count = 0;
        self.temp_easy_revert = None;
        self.retry_backup = None;
        self.pending_summary = None;
        self.is_index_savable = true;

        match source {
            LaunchSource::NewCampaign(diff) => {
                self.play_mode = PlayMode::Campaign;
                if let Some(prof) = self.save.active_profile_mut() {
                    prof.reset_campaign();
                    prof.difficulty = diff;
                    prof.savable_difficulty = diff;
                }
                self.save.save();
                self.start_campaign_level(0, true);
            }
            LaunchSource::ContinueCampaign => {
                self.play_mode = PlayMode::Campaign;
                let idx = if let Some(prof) = self.save.active_profile_mut() {
                    prof.difficulty = prof.savable_difficulty;
                    let len = self.assets.campaign_levels.len();
                    let next_idx = if prof.records.len() < len {
                        prof.records.len()
                    } else {
                        prof.level_index.min(len.saturating_sub(1))
                    };
                    prof.level_index = next_idx;
                    prof.savable_level_index = next_idx;
                    next_idx
                } else {
                    0
                };
                self.save.save();
                self.start_campaign_level(idx, true);
            }
            LaunchSource::Lotd => {
                self.play_mode = PlayMode::Lotd;
                self.lotd_index = 0;
                self.lotd_score = 0;
                self.lotd_level_scores.clear();
                self.show_status_toast(
                    "Couldn't contact the level server.  Loading cached levels.",
                );
                let def = self.assets.lotd_levels[0].clone();
                self.launch_level_def(def);
            }
            LaunchSource::CustomHub { play_focused } => {
                self.open_custom_hub(play_focused);
            }
            LaunchSource::Match { mirror } => {
                self.play_mode = PlayMode::Campaign;
                let idx = self
                    .save
                    .active_profile()
                    .map(|p| {
                        if mirror {
                            p.mirror_level_index.max(0) as usize
                        } else {
                            p.match_level_index.max(0) as usize
                        }
                    })
                    .unwrap_or(0);
                self.start_match_level(idx, mirror, true);
            }
            LaunchSource::RetryTrophy { idx, difficulty } => {
                self.play_mode = PlayMode::Campaign;
                self.is_index_savable = false;
                if let Some(prof) = self.save.active_profile_mut() {
                    self.retry_backup = prof.records.get(&idx).cloned();
                    prof.level_index = idx;
                    prof.difficulty = difficulty;
                }
                self.start_campaign_level(idx, false);
            }
            LaunchSource::Endless(cfg) => {
                self.launch_endless_level(cfg, 1, 0);
            }
        }
    }

    pub fn launch_endless_level(
        &mut self,
        config: EndlessConfig,
        level_num: usize,
        session_score: u32,
    ) {
        let gen_diff = config.pick_gen_difficulty(&mut self.rng);
        let play_diff = config.play_difficulty_for(gen_diff);
        let mut def = generate_custom_level_with_max_size(
            gen_diff,
            config.max_cols,
            config.max_rows,
            &mut self.rng,
        );
        def.index = level_num.saturating_sub(1);
        def.title = format!("Level {}", level_num);
        def.description = match config.rules {
            EndlessRules::MatchLevel => format!("Endless - {}", gen_diff.label()),
            EndlessRules::Fixed(d) => format!("{} ({})", gen_diff.label(), d.human_label()),
        };
        self.play_mode = PlayMode::Endless {
            config,
            level_num,
            pre_level_score: session_score,
            session_score,
            current_level: Box::new(def.clone()),
            current_difficulty: play_diff,
        };
        self.launch_level_def(def);
    }

    pub fn start_campaign_level(&mut self, idx: usize, notify_allowed: bool) {
        let total = self.assets.campaign_levels.len();
        if total == 0 {
            return;
        }
        let clamped = idx.min(total - 1);
        self.current_level_idx = clamped;
        let def = self.assets.campaign_levels[clamped].clone();

        if notify_allowed
            && self.play_mode == PlayMode::Campaign
            && clamped == 25
            && let Some(prof) = self.save.active_profile_mut()
            && prof.records.len() >= 25
            && !prof.awards[0]
        {
            prof.awards[0] = true;
            prof.match_level_index = 0;
            self.save.save();
            self.audio.play_sfx("BonusTwoSound");
            self.push_modal(ModalState::Award {
                kind: AwardKind::LightWater,
                then_champion: false,
                then_ultralux: false,
                then_unlock_match_level: Some(Box::new(def)),
            });
            return;
        }
        self.launch_level_def(def);
    }

    pub fn launch_level_def(&mut self, def: LevelDef) {
        let was_in_level = self.campaign_sim.is_some() || self.match_sim.is_some();
        let diff = match &self.play_mode {
            PlayMode::Lotd => Difficulty::Medium,
            PlayMode::CustomPack { difficulty, .. }
            | PlayMode::EditorTest { difficulty }
            | PlayMode::Endless {
                current_difficulty: difficulty,
                ..
            } => *difficulty,
            PlayMode::Campaign => self
                .save
                .active_profile()
                .map(|p| p.difficulty)
                .unwrap_or(Difficulty::Medium),
        };
        self.bg_anim.load(&def.background_id);
        self.audio.clear_delayed();
        self.audio
            .play_music(music_for_background(&def.background_id), true);

        let sim = LevelSim::new(def.clone(), diff);
        let tile_count = (def.rows * def.cols).max(1);
        let pop_end = 3.5 + tile_count as f32 * 0.1 + 0.6;
        self.board_pop_elapsed = Some((0.0, 3.5, pop_end, 0));
        self.board_hide = None;
        self.board_age = 0.0;
        self.pause_elapsed = 0.0;
        self.pause_fade = 1.0;
        self.tut_age = 0.0;
        self.last_tut_idx = 0;
        self.ff_hint_age = None;
        self.hud_anim.show_overlay();
        self.hud_tutorial_active = false;

        self.info_banner = Some(InfoBanner {
            title: def.title.clone(),
            description: def.description.clone(),
            elapsed: 0.0,
            delay: 0.0,
        });
        self.audio.play_sfx_delayed("SwapOneSound", 0.0, 1.0);
        self.audio.play_sfx_delayed("SwapTwoSound", 4.3, 1.0);

        self.campaign_sim = Some(sim);
        self.match_sim = None;
        self.summary_state = None;
        self.champion_state = None;
        self.blurbs.clear();
        self.leaks.clear();
        self.fireworks.clear();
        let base_score = self.display_base_score();
        if was_in_level {
            self.hud_score_roller.roll_to(base_score, 1.0, 0.0);
            self.hud_moves_roller.roll_to(0, 1.0, 0.0);
            self.hud_time_roller.roll_to(0, 1.0, 0.0);
        } else {
            self.hud_score_roller = ScoreRoller::new(base_score);
            self.hud_moves_roller = ScoreRoller::new(0);
            self.hud_time_roller = ScoreRoller::new(0);
        }
        self.screen = Screen::Playing;
    }

    pub fn start_match_level(&mut self, idx: usize, mirror: bool, first_entry: bool) {
        let was_in_level = self.campaign_sim.is_some() || self.match_sim.is_some();
        let sim = MatchSim::new(idx, mirror, &mut self.rng);
        self.bg_anim.load(&sim.level.background_id);
        if first_entry {
            self.audio.clear_delayed();
        }
        self.audio
            .play_music(music_for_background(&sim.level.background_id), true);

        let tile_count = (sim.level.rows * sim.level.cols).max(1);
        let pop_end = 1.5 + tile_count as f32 * 0.1 + 0.6;
        self.board_pop_elapsed = Some((0.0, 1.5, pop_end, 0));
        self.board_hide = None;
        self.board_age = 0.0;
        self.pause_elapsed = 0.0;
        self.pause_fade = 1.0;
        self.hud_anim.show_overlay();
        self.hud_tutorial_active = false;

        if idx == 0 && first_entry {
            self.info_banner = Some(InfoBanner {
                title: "Match Mode".to_string(),
                description: "Ready, set, go!".to_string(),
                elapsed: 0.0,
                delay: 0.0,
            });
            self.audio.play_sfx_delayed("SwapOneSound", 0.0, 1.0);
            self.audio.play_sfx_delayed("SwapTwoSound", 4.3, 1.0);
        } else if !first_entry {
            let pts = if mirror { 350 } else { 200 };
            self.play_random_bonus_sfx();
            self.blurbs.push(ScoreBlurbFx {
                x: 730.0,
                y: 570.0,
                text: pts.to_string(),
                age: 0.0,
            });
        }

        self.match_sim = Some(sim);
        self.campaign_sim = None;
        self.summary_state = None;
        self.champion_state = None;
        self.leaks.clear();
        self.fireworks.clear();
        let base_score = self.display_base_score();
        if was_in_level {
            self.hud_score_roller.roll_to(base_score, 1.0, 0.0);
        } else {
            self.hud_score_roller = ScoreRoller::new(base_score);
        }
        self.hud_moves_roller = ScoreRoller::new(0);
        self.hud_time_roller = ScoreRoller::new(0);
        self.screen = Screen::MatchPlaying;
    }

    pub(crate) fn repeat_current_level(&mut self) {
        match self.play_mode.clone() {
            PlayMode::Lotd => {
                let def = self.assets.lotd_levels[self.lotd_index].clone();
                self.launch_level_def(def);
            }
            PlayMode::CustomPack {
                pack_id,
                level_idx,
                difficulty,
                session_score,
            } => {
                self.launch_custom_pack_level(&pack_id, level_idx, difficulty, session_score);
            }
            PlayMode::EditorTest { .. } => {
                if let Some(def) = self.editor.as_ref().map(|e| e.draft.clone()) {
                    self.launch_level_def(def);
                }
            }
            PlayMode::Endless { current_level, .. } => {
                self.launch_level_def(*current_level);
            }
            PlayMode::Campaign => {
                self.start_campaign_level(self.current_level_idx, false);
            }
        }
    }

    pub(crate) fn return_to_main_menu(&mut self) {
        if matches!(self.play_mode, PlayMode::EditorTest { .. }) && self.editor.is_some() {
            self.return_to_editor();
            return;
        }
        let was_custom = matches!(self.play_mode, PlayMode::CustomPack { .. });
        self.temp_easy_revert = None;
        self.pending_summary = None;
        self.is_index_savable = true;
        if self.play_mode == PlayMode::Campaign {
            self.revert_retry_if_needed();
            if let Some(prof) = self.save.active_profile_mut() {
                prof.level_index = prof.savable_level_index;
                prof.difficulty = prof.savable_difficulty;
            }
            self.save.save();
        }
        self.play_mode = PlayMode::Campaign;
        self.campaign_sim = None;
        self.match_sim = None;
        self.summary_state = None;
        self.champion_state = None;
        self.board_pop_elapsed = None;
        self.board_hide = None;
        self.board_age = 0.0;
        self.pause_elapsed = 0.0;
        self.pause_fade = 1.0;
        self.tut_age = 0.0;
        self.last_tut_idx = 0;
        self.ff_hint_age = None;
        self.hud_anim = crate::ui::hud::HudAnimState::default();
        self.hud_tutorial_active = false;
        self.hud_moves_roller = ScoreRoller::new(0);
        self.hud_time_roller = ScoreRoller::new(0);
        self.info_banner = None;
        self.leaks.clear();
        self.fireworks.clear();
        self.blurbs.clear();
        self.audio.clear_delayed();
        self.audio.stop_music();
        self.audio.play_music("LittleRayMusic", true);
        if was_custom {
            self.bg_anim.lighten(0.0);
            self.bg_anim.load("NightHills");
            self.custom_hub.clamp_selection(&self.custom_packs);
            self.screen = Screen::CustomHub;
        } else {
            self.bg_anim.lighten(2.9);
            self.bg_anim.load("NightHills");
            self.enter_title_screen();
        }
    }

    pub(crate) fn handle_sim_events(&mut self, events: Vec<SimEvent>, layout: BoardLayout) {
        for ev in events {
            match ev {
                SimEvent::PlaySfx(name) => self.audio.play_sfx(name),
                SimEvent::PlaySfxDelayed(name, delay) => {
                    self.audio.play_sfx_delayed(name, delay, 1.0);
                }
                SimEvent::PlayRandomBonusSfx => self.play_random_bonus_sfx(),
                SimEvent::ScoreBlurb(gx, gy, pts) => {
                    self.blurbs.push(ScoreBlurbFx {
                        x: layout.origin_x + gx * layout.tile_px,
                        y: layout.origin_y + gy * layout.tile_px,
                        text: pts.to_string(),
                        age: 0.0,
                    });
                }
                SimEvent::LeakErupt(tx, ty, exit_side, color) => {
                    let (_, cy) = layout.tile_center(tx, ty);
                    let half = layout.tile_px * 0.5;
                    let (_, dy) = exit_side.delta();
                    let ey = cy + dy as f32 * half;
                    self.leaks
                        .push(LeakEmitter::new((tx, ty), ey, exit_side, color));
                }
                SimEvent::LevelWon(res) => self.on_campaign_win(res),
                SimEvent::LevelLost => self.on_campaign_fail(),
                SimEvent::MatchWon => self.on_match_win(),
            }
        }
    }
}
