// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::render::{BoardAnim, BoardLayout, Canvas, draw_cursor, draw_fx_layers};
use crate::ui::champion::{build_champion_buttons, render_champion_screen};
use crate::ui::summary::{build_summary_buttons, render_summary_screen};
use crate::ui::widgets::UiButton;
use crate::ui::{GameApp, Screen, hud, modals, splash, title};

impl GameApp {
    pub(crate) fn current_buttons(&self) -> Vec<UiButton> {
        if let Some(top) = self.modals.last() {
            return modals::build_modal_buttons(
                top,
                &self.assets,
                &self.renderer.bank,
                &self.renderer.fonts,
                &self.save,
            );
        }
        match self.screen {
            Screen::Splash => Vec::new(),
            Screen::Title => title::build_title_buttons(
                self.title_submenu,
                &self.save,
                self.title_age,
                self.title_hiding.map(|h| h.0),
            ),
            Screen::Playing | Screen::MatchPlaying => hud::build_playing_buttons(
                self.campaign_sim.as_ref(),
                self.match_sim.as_ref(),
                self.is_board_busy(),
            ),
            Screen::Paused => modals::build_pause_buttons(),
            Screen::Summary => self
                .summary_state
                .as_ref()
                .map(build_summary_buttons)
                .unwrap_or_default(),
            Screen::Champion => self
                .champion_state
                .as_ref()
                .map(build_champion_buttons)
                .unwrap_or_default(),
        }
    }

    pub fn render(&mut self, canvas: &mut Canvas) {
        if self.screen == Screen::Splash {
            splash::render_splash(canvas, &self.renderer.bank, self.splash_age);
            if self.cursor_in_window {
                draw_cursor(
                    canvas,
                    &self.renderer.bank,
                    self.current_cursor(),
                    self.mouse_pos.0,
                    self.mouse_pos.1,
                );
            }
            return;
        }

        if self.screen == Screen::Title {
            title::render_title_screen(
                canvas,
                &self.assets,
                &self.renderer.bank,
                &self.renderer.backgrounds,
                &self.renderer.fonts,
                &self.bg_anim,
                &self.save,
                self.title_submenu,
                self.title_age,
                self.title_submenu_age,
                self.title_hiding.map(|h| h.0),
                self.mouse_pos,
            );
        } else if self.screen == Screen::Champion {
            self.renderer
                .draw_background(canvas, &self.assets, &self.bg_anim);
            if let Some(ch) = &self.champion_state {
                render_champion_screen(
                    canvas,
                    &self.renderer.bank,
                    &self.renderer.fonts,
                    ch,
                    self.mouse_pos,
                );
            }
        } else {
            self.renderer
                .draw_background(canvas, &self.assets, &self.bg_anim);
            let keep_paused_dim = self.screen == Screen::Paused;
            if self.campaign_sim.is_some()
                && (self.screen != Screen::Summary || self.board_hide.is_some())
            {
                self.render_campaign(canvas, keep_paused_dim);
            } else if self.match_sim.is_some() {
                self.render_match(canvas, keep_paused_dim);
            }
            if self.screen == Screen::Paused {
                modals::render_pause_menu(canvas, &self.renderer.fonts, self.mouse_pos);
            } else if self.screen == Screen::Summary
                && let Some(sum) = &self.summary_state
            {
                render_summary_screen(
                    canvas,
                    &self.renderer.bank,
                    &self.renderer.fonts,
                    sum,
                    self.mouse_pos,
                );
            }
        }

        let fw_alpha = self
            .summary_state
            .as_ref()
            .and_then(|s| s.hiding)
            .map(|(h, _)| (1.0 - h).clamp(0.0, 1.0))
            .unwrap_or(1.0);
        draw_fx_layers(
            canvas,
            &self.renderer.fonts,
            &self.fireworks,
            &self.blurbs,
            fw_alpha,
        );
        if let Some(banner) = &self.info_banner {
            hud::draw_level_info_screen(
                canvas,
                &self.renderer.fonts,
                &banner.title,
                &banner.description,
                banner.elapsed,
                banner.delay,
            );
        }
        if let Some((txt, age)) = &self.status_toast {
            hud::draw_status_toast(canvas, &self.renderer.fonts, txt, *age);
        }
        if !self.modals.is_empty() {
            modals::render_modal_stack(
                canvas,
                &self.assets,
                &self.renderer.bank,
                &self.renderer.fonts,
                &self.save,
                &self.modals,
                self.time,
                self.mouse_pos,
            );
        }
        if self.cursor_in_window {
            draw_cursor(
                canvas,
                &self.renderer.bank,
                self.current_cursor(),
                self.mouse_pos.0,
                self.mouse_pos.1,
            );
        }
    }

    fn current_board_anim(&self) -> BoardAnim {
        if let Some((elapsed, delay, _, _)) = self.board_hide {
            BoardAnim::HideDrop { elapsed, delay }
        } else if let Some((elapsed, start_delay, _, _)) = self.board_pop_elapsed {
            BoardAnim::PopIn {
                elapsed,
                start_delay,
            }
        } else if self.screen == Screen::Paused {
            BoardAnim::Obscured {
                alpha: self.pause_fade,
            }
        } else {
            BoardAnim::Active
        }
    }

    fn render_campaign(&mut self, canvas: &mut Canvas, paused: bool) {
        let board_anim = self.current_board_anim();
        let Some(sim) = self.campaign_sim.as_ref() else {
            return;
        };
        let layout =
            BoardLayout::compute(sim.level.cols, sim.level.rows, hud::campaign_board_area());
        let hover = if self.modals.is_empty() && !paused {
            layout.hit_test(self.mouse_pos.0, self.mouse_pos.1)
        } else {
            None
        };
        let swap_elig = sim
            .selected
            .map(|s| sim.get_swap_eligible(s))
            .unwrap_or_default();
        let tut_tiles = sim
            .active_tutorial()
            .map(|t| t.tiles.as_slice())
            .unwrap_or(&[]);

        self.renderer.draw_board(
            canvas,
            &sim.tiles,
            layout,
            sim.selected,
            &swap_elig,
            hover,
            tut_tiles,
            &self.leaks,
            self.save.config.color_blind,
            false,
            board_anim,
            self.time,
        );
        if !paused {
            hud::render_campaign_hud(
                canvas,
                &self.renderer.bank,
                &self.renderer.fonts,
                sim,
                layout,
                self.hud_score_roller.value(),
                self.tut_age,
                self.mouse_pos,
                self.is_board_busy(),
            );
        }
    }

    fn render_match(&mut self, canvas: &mut Canvas, paused: bool) {
        let board_anim = self.current_board_anim();
        let Some(sim) = self.match_sim.as_ref() else {
            return;
        };
        let play_layout =
            BoardLayout::compute(sim.level.cols, sim.level.rows, hud::match_play_area());
        let hover = if self.modals.is_empty() && !paused {
            play_layout.hit_test(self.mouse_pos.0, self.mouse_pos.1)
        } else {
            None
        };
        let swap_elig = sim
            .selected
            .map(|s| sim.get_swap_eligible(s))
            .unwrap_or_default();

        self.renderer.draw_board(
            canvas,
            &sim.play_tiles,
            play_layout,
            sim.selected,
            &swap_elig,
            hover,
            &[],
            &[],
            self.save.config.color_blind,
            false,
            board_anim,
            self.time,
        );

        if !paused && self.board_hide.is_none() {
            let preview_alpha = if let Some((elapsed, _, _, _)) = self.board_pop_elapsed {
                BoardAnim::Obscured {
                    alpha: ((elapsed - 1.0) / 1.0).clamp(0.0, 1.0),
                }
            } else {
                BoardAnim::Active
            };
            let goal_layout =
                BoardLayout::compute(sim.level.cols, sim.level.rows, hud::match_preview_area());
            self.renderer.draw_board(
                canvas,
                &sim.goal_tiles,
                goal_layout,
                None,
                &[],
                None,
                &[],
                &[],
                self.save.config.color_blind,
                sim.mirror_mode,
                preview_alpha,
                self.time,
            );
        }
        if !paused {
            hud::render_match_hud(
                canvas,
                &self.renderer.bank,
                &self.renderer.fonts,
                sim,
                self.hud_score_roller.value(),
                self.mouse_pos,
                self.is_board_busy(),
            );
        }
    }
}
