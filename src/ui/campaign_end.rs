// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::{AwardKind, Difficulty, MetricResult};
use crate::render::FireworkBurst;
use crate::ui::champion::ChampionState;
use crate::ui::summary::{SummaryHideAction, SummaryState};
use crate::ui::widgets::{ModalState, QuestionId};
use crate::ui::{GameApp, HideAfterBoard, InfoBanner, PlayMode, Screen};
use rand::RngExt;

impl GameApp {
    pub(crate) fn on_campaign_win(&mut self, res: MetricResult) {
        let (moves, time_ms, rows, cols) = self
            .campaign_sim
            .as_ref()
            .map(|s| (s.moves, s.elapsed_ms as u32, s.level.rows, s.level.cols))
            .unwrap_or((0, 0, 5, 5));

        let old_total = self.display_base_score();
        let new_total = match &mut self.play_mode {
            PlayMode::Lotd => {
                if self.lotd_level_scores.len() <= self.lotd_index {
                    self.lotd_level_scores.resize(self.lotd_index + 1, 0);
                }
                let prev = self.lotd_level_scores[self.lotd_index];
                self.lotd_level_scores[self.lotd_index] = prev.max(res.total_score);
                self.lotd_score = self.lotd_level_scores.iter().sum();
                self.lotd_score
            }
            PlayMode::CustomPack {
                pack_id,
                level_idx,
                session_score,
                ..
            } => {
                self.custom_packs
                    .record_win(pack_id, *level_idx, res, moves, time_ms);
                *session_score += res.total_score;
                *session_score
            }
            PlayMode::EditorTest { .. } => res.total_score,
            PlayMode::Endless {
                pre_level_score,
                session_score,
                ..
            } => {
                let next = *pre_level_score + res.total_score;
                *session_score = next;
                next
            }
            PlayMode::Campaign => {
                if let Some(prof) = self.save.active_profile_mut() {
                    if self.retry_backup.is_none() {
                        self.retry_backup = prof.records.get(&self.current_level_idx).cloned();
                    }
                    prof.record_win(self.current_level_idx, res, moves, time_ms);
                    if self.is_index_savable {
                        prof.savable_level_index = (self.current_level_idx + 1)
                            .min(self.assets.campaign_levels.len().saturating_sub(1));
                    }
                    prof.total_score()
                } else {
                    res.total_score
                }
            }
        };

        let drop_dur = (0.15 * (rows * cols) as f32).min(1.8) + 1.2;
        self.board_hide = Some((0.0, 0.0, drop_dur, HideAfterBoard::None));
        self.hud_anim.hide_skip_tutorial();
        self.hud_anim.hide_overlay();
        self.hud_tutorial_active = false;
        self.audio.play_music_delayed("GlideSuccess", 0.6, true);
        self.bg_anim.darken(0.0);
        self.screen = Screen::Summary;

        let trigger_ultralux = if self.play_mode == PlayMode::Campaign {
            let total_lvls = self.assets.campaign_levels.len();
            if let Some(prof) = self.save.active_profile_mut()
                && !prof.awards[3]
                && prof.all_platinum(total_lvls)
            {
                prof.awards[3] = true;
                self.save.save();
                true
            } else {
                false
            }
        } else {
            false
        };

        if trigger_ultralux {
            self.summary_state = None;
            self.pending_summary = Some((res, old_total, new_total));
            self.audio.play_sfx("BonusTwoSound");
            self.push_modal(ModalState::Award {
                kind: AwardKind::Ultralux,
                then_champion: false,
                then_ultralux: false,
                then_unlock_match_level: None,
            });
        } else {
            self.show_summary(res, old_total, new_total);
        }
    }

    pub(crate) fn show_summary(&mut self, res: MetricResult, old_total: u32, new_total: u32) {
        self.audio.play_sfx_delayed("SwapTwoSound", 0.2, 1.0);
        self.summary_state = Some(SummaryState::new(res, old_total, new_total));
    }

    pub(crate) fn on_campaign_fail(&mut self) {
        self.fail_count += 1;
        self.audio.clear_delayed();
        self.audio.stop_music();

        let (rows, cols) = self
            .campaign_sim
            .as_ref()
            .map(|s| (s.level.rows, s.level.cols))
            .unwrap_or((5, 5));
        let drop_end = 1.0 + (0.15 * (rows * cols) as f32).min(1.8) + 1.2;
        let info_end = 1.0 + 4.8;
        let total_wait = drop_end.max(info_end);

        self.info_banner = Some(InfoBanner {
            title: "Whoops!".to_string(),
            description: "Try again...".to_string(),
            elapsed: 0.0,
            delay: 1.0,
        });
        self.audio.play_sfx_delayed("SwapOneSound", 1.0, 1.0);
        self.audio.play_sfx_delayed("SwapTwoSound", 5.3, 1.0);

        let diff = self
            .save
            .active_profile()
            .map(|p| p.difficulty)
            .unwrap_or(Difficulty::Medium);
        let offer_easy = self.fail_count == 2
            && diff != Difficulty::Easy
            && self.play_mode == PlayMode::Campaign;
        self.board_hide = Some((
            0.0,
            1.0,
            total_wait,
            if offer_easy {
                HideAfterBoard::OfferEasyDifficulty
            } else {
                HideAfterBoard::RepeatLevel
            },
        ));
    }

    pub(crate) fn on_match_win(&mut self) {
        let (next_num, rows, cols) = self
            .match_sim
            .as_ref()
            .map(|m| (m.level_index + 2, m.level.rows, m.level.cols))
            .unwrap_or((2, 3, 3));
        self.info_banner = Some(InfoBanner {
            title: "NICE JOB!".to_string(),
            description: format!("Get ready for level {}", next_num),
            elapsed: 0.0,
            delay: 0.0,
        });
        self.audio.play_sfx_delayed("SwapOneSound", 0.0, 1.0);
        self.audio.play_sfx_delayed("SwapTwoSound", 4.3, 1.0);
        let drop_end = (0.15 * (rows * cols) as f32).min(1.8) + 1.2;
        self.board_hide = Some((0.0, 0.0, drop_end, HideAfterBoard::AdvanceMatch));
    }

    pub(crate) fn advance_after_summary(&mut self) {
        self.fail_count = 0;
        if let Some(prev) = self.temp_easy_revert.take()
            && let Some(prof) = self.save.active_profile_mut()
        {
            prof.difficulty = prev;
        }
        self.revert_retry_if_needed();

        match self.play_mode.clone() {
            PlayMode::Lotd => {
                self.lotd_index += 1;
                if self.lotd_index >= self.assets.lotd_levels.len() {
                    self.show_status_toast("Check back soon for new levels of the day!");
                    self.return_to_main_menu();
                } else {
                    let def = self.assets.lotd_levels[self.lotd_index].clone();
                    self.launch_level_def(def);
                }
                return;
            }
            PlayMode::CustomPack {
                pack_id,
                level_idx,
                session_score,
                difficulty,
            } => {
                let next = level_idx + 1;
                let pack_info = self
                    .custom_packs
                    .packs
                    .iter()
                    .find(|p| p.id == pack_id)
                    .map(|p| (p.pack.meta.name.clone(), p.pack.levels.len()));
                if let Some((pack_name, len)) = pack_info {
                    if next < len {
                        self.launch_custom_pack_level(&pack_id, next, difficulty, session_score);
                    } else {
                        self.show_status_toast(&format!("Completed pack '{}'!", pack_name));
                        self.return_to_main_menu();
                    }
                } else {
                    self.return_to_main_menu();
                }
                return;
            }
            PlayMode::EditorTest { .. } => {
                self.show_status_toast("Playtest complete: back in editor.");
                self.return_to_editor();
                return;
            }
            PlayMode::Endless {
                config,
                level_num,
                session_score,
                ..
            } => {
                self.launch_endless_level(config, level_num + 1, session_score);
                return;
            }
            PlayMode::Campaign => {}
        }

        let total = self.assets.campaign_levels.len();
        let next_idx = self.current_level_idx + 1;
        if next_idx < total {
            if let Some(prof) = self.save.active_profile_mut() {
                prof.level_index = next_idx;
                if self.is_index_savable {
                    prof.savable_level_index = next_idx;
                }
            }
            self.save.save();
            self.start_campaign_level(next_idx, true);
            return;
        }

        // Story Mode completed!
        let (need_champ_award, need_ultra_award, score, has_ultra) =
            if let Some(prof) = self.save.active_profile_mut() {
                let ap = prof.all_platinum(total);
                prof.level_index = 0;
                if self.is_index_savable {
                    prof.savable_level_index = 0;
                }
                let nc = !prof.awards[1];
                if nc {
                    prof.awards[1] = true;
                    prof.mirror_level_index = 0;
                }
                let nu = ap && !prof.awards[3];
                if nu {
                    prof.awards[3] = true;
                }
                (nc, nu, prof.total_score(), prof.awards[3])
            } else {
                (false, false, 0, false)
            };
        self.save.save();

        if need_champ_award {
            self.audio.play_sfx("BonusTwoSound");
            self.push_modal(ModalState::Question {
                id: QuestionId::UnlockMirrorNotice(need_ultra_award),
                message: "-SECRET UNLOCKED!-\n\nMIRROR MATCH has been unlocked on the main menu!"
                    .to_string(),
                yes_label: "Ok",
                no_label: None,
            });
        } else if need_ultra_award {
            self.audio.play_sfx("BonusTwoSound");
            self.push_modal(ModalState::Award {
                kind: AwardKind::Ultralux,
                then_champion: true,
                then_ultralux: false,
                then_unlock_match_level: None,
            });
        } else {
            self.enter_champion_screen(score, has_ultra);
        }
    }

    pub(crate) fn advance_after_match(&mut self) {
        let is_mirror = self.match_sim.as_ref().is_some_and(|m| m.mirror_mode);
        let mut trigger_mirror_award = false;
        let next_idx = if let Some(prof) = self.save.active_profile_mut() {
            if is_mirror {
                prof.mirror_level_index = prof.mirror_level_index.max(0) + 1;
                if prof.mirror_level_index == 50 && !prof.awards[2] {
                    prof.awards[2] = true;
                    trigger_mirror_award = true;
                }
                prof.mirror_level_index.max(0) as usize
            } else {
                prof.match_level_index = prof.match_level_index.max(0) + 1;
                prof.match_level_index.max(0) as usize
            }
        } else {
            1
        };
        self.save.save();
        if trigger_mirror_award {
            self.info_banner = None;
            self.audio.play_sfx("BonusTwoSound");
            self.push_modal(ModalState::Award {
                kind: AwardKind::MirrorMaster,
                then_champion: false,
                then_ultralux: false,
                then_unlock_match_level: None,
            });
        } else {
            self.start_match_level(next_idx, is_mirror, false);
        }
    }

    pub(crate) fn enter_champion_screen(&mut self, score: u32, has_ultra: bool) {
        self.campaign_sim = None;
        self.match_sim = None;
        self.summary_state = None;
        self.hud_anim = crate::ui::hud::HudAnimState::default();
        self.hud_tutorial_active = false;
        self.screen = Screen::Champion;
        self.champion_state = Some(ChampionState::new(score, has_ultra, &mut self.rng));
        self.audio.clear_delayed();
        self.audio.stop_music();
        self.audio.play_music("IstanbulMusic", true);
        self.audio.play_sfx("BigWhooshSound");
        self.audio.play_sfx_delayed("BigWhooshSound", 0.9, 1.0);
        self.audio.play_sfx_delayed("BigWhooshSound", 1.7, 1.0);
        self.audio.play_sfx_delayed("SwishSound", 3.0, 1.0);
        self.audio.play_sfx_delayed("SwishSound", 3.8, 1.0);
        self.audio.play_sfx_delayed("SwishSound", 4.6, 1.0);
        self.audio.play_sfx_delayed("FireworkOneSound", 5.2, 1.0);
    }

    pub(crate) fn revert_retry_if_needed(&mut self) {
        let Some(backup) = self.retry_backup.take() else {
            return;
        };
        let Some(prof) = self.save.active_profile_mut() else {
            return;
        };
        let cur = prof.records.get(&self.current_level_idx).cloned();
        let keep_old = match cur {
            None => true,
            Some(c) => {
                backup.best_trophy > c.best_trophy
                    || (backup.best_trophy == c.best_trophy && backup.best_score >= c.best_score)
            }
        };
        if keep_old {
            prof.records.insert(self.current_level_idx, backup);
        }
    }

    pub(crate) fn spawn_firework(&mut self, fx: f32, fy: f32) {
        const FIREWORK_COLORS: [(u8, u8, u8); 3] =
            [(0xFF, 0x00, 0x00), (0x00, 0xFF, 0x00), (0x00, 0x00, 0xFF)];
        let rgb = FIREWORK_COLORS[self.rng.random_range(0..FIREWORK_COLORS.len())];
        let power = self.rng.random_range(1.0..1.2);
        self.fireworks
            .push(FireworkBurst::new(fx, fy, rgb, power, &mut self.rng));
        let snd = if self.rng.random_bool(0.5) {
            "FireworkOneSound"
        } else {
            "FireworkTwoSound"
        };
        self.audio.play_sfx_vol(snd, 0.4);
    }

    pub(crate) fn display_base_score(&self) -> u32 {
        match &self.play_mode {
            PlayMode::Lotd => self.lotd_score,
            PlayMode::CustomPack { session_score, .. }
            | PlayMode::Endless { session_score, .. } => *session_score,
            PlayMode::EditorTest { .. } => 0,
            PlayMode::Campaign => self
                .save
                .active_profile()
                .map(|p| p.total_score())
                .unwrap_or(0),
        }
    }

    pub(crate) fn play_random_bonus_sfx(&mut self) {
        self.audio.play_sfx(if self.rng.random_bool(0.5) {
            "BonusOneSound"
        } else {
            "BonusTwoSound"
        });
    }

    pub(crate) fn show_status_toast(&mut self, text: &str) {
        self.status_toast = Some((text.to_string(), 0.0));
    }

    pub(crate) fn start_summary_hide(&mut self, action: SummaryHideAction) {
        let already = self
            .summary_state
            .as_ref()
            .is_some_and(|s| s.hiding.is_some());
        if let Some(sum) = &mut self.summary_state {
            sum.start_hide(action);
        } else {
            self.execute_summary_action(action);
            return;
        }
        if !already {
            let more = match &self.play_mode {
                PlayMode::Lotd | PlayMode::Endless { .. } => true,
                PlayMode::EditorTest { .. } => false,
                PlayMode::Campaign => {
                    self.current_level_idx + 1 < self.assets.campaign_levels.len()
                }
                PlayMode::CustomPack {
                    pack_id, level_idx, ..
                } => self
                    .custom_packs
                    .packs
                    .iter()
                    .find(|p| p.id == *pack_id)
                    .is_some_and(|p| level_idx + 1 < p.pack.levels.len()),
            };
            if action == SummaryHideAction::RetryLevel
                || (action == SummaryHideAction::NextLevel && more)
            {
                self.bg_anim.lighten(0.0);
            }
        }
    }

    fn execute_summary_action(&mut self, action: SummaryHideAction) {
        self.summary_state = None;
        self.fireworks.clear();
        match action {
            SummaryHideAction::NextLevel => self.advance_after_summary(),
            SummaryHideAction::RetryLevel => {
                if self.play_mode == PlayMode::Campaign
                    && let Some(prof) = self.save.active_profile()
                {
                    self.retry_backup = prof.records.get(&self.current_level_idx).cloned();
                }
                if let PlayMode::Endless {
                    pre_level_score,
                    session_score,
                    ..
                } = &mut self.play_mode
                {
                    *session_score = *pre_level_score;
                }
                self.repeat_current_level();
            }
            SummaryHideAction::MainMenu => self.return_to_main_menu(),
        }
    }

    pub(crate) fn tick_end_screens(&mut self, dt: f32) {
        if self.screen == Screen::Summary {
            let (fire_tick, done_act) = self
                .summary_state
                .as_mut()
                .map_or((false, None), |s| s.update(dt));
            if fire_tick && self.rng.random_range(0..8) == 0 {
                let (fx, fy) = (
                    self.rng.random_range(0.0..800.0),
                    self.rng.random_range(0.0..180.0),
                );
                self.spawn_firework(fx, fy);
            }
            if let Some(act) = done_act {
                self.execute_summary_action(act);
            }
        } else if self.screen == Screen::Champion
            && let Some(ch) = &mut self.champion_state
            && let Some(quit) = ch.update(dt)
        {
            if quit {
                self.should_quit = true;
            } else {
                self.return_to_main_menu();
            }
        }
    }
}
