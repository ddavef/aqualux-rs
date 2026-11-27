// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::render::{
    BoardAnim, BoardLayout, Canvas, draw_cursor, draw_fireworks, draw_score_blurbs,
};
use crate::ui::champion::{build_champion_buttons, render_champion_screen};
use crate::ui::summary::{build_summary_buttons, render_summary_screen};
use crate::ui::widgets::UiButton;
use crate::ui::{GameApp, PlayMode, Screen, custom_hub, editor, hud, modals, splash, title};

impl GameApp {
    pub(crate) fn current_buttons(&self) -> Vec<UiButton> {
        if let Some(top) = self.modals.last() {
            return modals::build_modal_buttons(
                top,
                &self.assets,
                &self.renderer.bank,
                &self.renderer.fonts,
                &self.save,
                self.hdr_available,
            );
        }
        match self.screen {
            Screen::Splash => Vec::new(),
            Screen::Title => title::build_title_buttons(
                self.title_submenu,
                &self.save,
                &self.renderer.fonts,
                self.title_age,
                self.title_menu_anim,
                self.title_profile_anim,
                self.title_wait_profile_latched,
                self.title_hiding.as_ref().map(|h| h.0),
            ),
            Screen::CustomHub => {
                custom_hub::build_custom_hub_buttons(&self.custom_packs, &self.custom_hub)
            }
            Screen::Editor => self
                .editor
                .as_ref()
                .map(editor::build_editor_buttons)
                .unwrap_or_default(),
            Screen::Playing | Screen::MatchPlaying => hud::build_playing_buttons(
                self.campaign_sim.as_ref(),
                self.match_sim.as_ref(),
                self.is_board_busy(),
                &self.hud_anim,
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
                self.title_menu_anim,
                self.title_profile_anim,
                self.title_wait_profile_latched,
                self.title_hiding.as_ref().map(|h| h.0),
                !self.modals.is_empty(),
                self.mouse_pos,
            );
            if let Some(ch) = &self.champion_state {
                render_champion_screen(
                    canvas,
                    &self.renderer.bank,
                    &self.renderer.fonts,
                    ch,
                    self.mouse_pos,
                );
            }
        } else if self.screen == Screen::CustomHub {
            self.renderer
                .draw_background(canvas, &self.assets, &self.bg_anim);
            custom_hub::render_custom_hub(
                canvas,
                &self.renderer.bank,
                &self.renderer.fonts,
                &self.custom_packs,
                &self.custom_hub,
                self.mouse_pos,
            );
        } else if self.screen == Screen::Editor {
            self.renderer
                .draw_background(canvas, &self.assets, &self.bg_anim);
            if let Some(ed) = &self.editor {
                editor::render_editor(
                    canvas,
                    &mut self.renderer,
                    ed,
                    self.save.config.color_blind,
                    self.time,
                    self.mouse_pos,
                    !self.modals.is_empty(),
                );
            }
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
                && (self.screen != Screen::Summary
                    || self.board_hide.is_some()
                    || self.hud_anim.overlay_alpha > 0.004)
            {
                self.render_campaign(canvas, keep_paused_dim);
            } else if self.match_sim.is_some() {
                self.render_match(canvas, keep_paused_dim);
            }
            if self.screen == Screen::Paused {
                let exit_label = match self.play_mode {
                    PlayMode::EditorTest { .. } => "Back to Editor",
                    PlayMode::CustomPack { .. } => "Custom Packs",
                    _ => "Main Menu",
                };
                let pause_mouse = if self.modals.is_empty() {
                    self.mouse_pos
                } else {
                    (-999.0, -999.0)
                };
                modals::render_pause_menu(
                    canvas,
                    &self.renderer.fonts,
                    exit_label,
                    pause_mouse,
                    self.pause_elapsed,
                );
            } else if self.screen == Screen::Summary
                && let Some(sum) = &self.summary_state
            {
                let fw_alpha = sum
                    .hiding
                    .map(|(h, _)| 1.0 - crate::render::easing::quad_out(h.clamp(0.0, 1.0)))
                    .unwrap_or(1.0);
                draw_fireworks(canvas, &self.renderer.bank, &self.fireworks, fw_alpha);
                render_summary_screen(
                    canvas,
                    &self.renderer.bank,
                    &self.renderer.fonts,
                    sum,
                    self.mouse_pos,
                );
            }
        }

        draw_score_blurbs(canvas, &self.renderer.fonts, &self.blurbs);
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
            if self.screen == Screen::Editor {
                hud::draw_editor_toast(canvas, &self.renderer.fonts, txt, *age);
            } else {
                hud::draw_status_toast(canvas, &self.renderer.fonts, txt, *age);
            }
        }
        if !self.modals.is_empty() {
            let win_chk = !self
                .options_pending_fullscreen
                .unwrap_or(self.save.config.fullscreen);
            let hdr_chk = self
                .hdr_available
                && self.options_pending_hdr.unwrap_or(self.save.config.hdr);
            modals::render_modal_stack(
                canvas,
                &self.assets,
                &self.renderer.bank,
                &self.renderer.fonts,
                &self.save,
                &self.modals,
                &self.modal_ages,
                self.trophy_hover_age,
                win_chk,
                hdr_chk,
                self.hdr_available,
                self.time,
                self.mouse_pos,
            );
        }
        if let Some(sa) = self.splash_overlay_age {
            splash::render_splash_overlay(canvas, &self.renderer.bank, sa);
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
        let hover = if self.modals.is_empty()
            && !paused
            && self.screen != Screen::Summary
            && !self.is_board_busy()
            && sim.is_ready
            && (sim.is_started || sim.active_tutorial().is_some())
            && !sim.active_tutorial().is_some_and(|t| t.click_to_continue())
        {
            layout.hit_test(self.mouse_pos.0, self.mouse_pos.1)
        } else {
            None
        };
        let swap_elig = sim
            .selected
            .map(|s| sim.get_swap_eligible(s))
            .unwrap_or_default();
        let color_blind = self
            .options_board_color_blind
            .unwrap_or(self.save.config.color_blind);

        if self.screen != Screen::Summary || self.board_hide.is_some() {
            self.renderer.draw_board(
                canvas,
                &sim.tiles,
                layout,
                sim.selected,
                &swap_elig,
                hover,
                sim.select_age,
                &self.leaks,
                color_blind,
                false,
                board_anim,
                self.board_age,
            );
        }
        if self.hud_anim.overlay_alpha > 0.004
            || (!paused && self.screen != Screen::Summary && sim.active_tutorial().is_some())
        {
            hud::render_campaign_hud(
                canvas,
                &self.renderer.bank,
                &self.renderer.fonts,
                sim,
                layout,
                self.hud_score_roller.value(),
                self.hud_moves_roller.value(),
                self.hud_time_roller.value(),
                self.tut_age,
                self.ff_hint_age,
                self.mouse_pos,
                self.is_board_busy(),
                self.play_mode == PlayMode::Campaign,
                &self.hud_anim,
                !paused && self.screen != Screen::Summary,
            );
        }
    }

    pub(crate) fn match_preview_alpha(&self) -> f32 {
        if let Some((elapsed, _, _, _)) = self.board_hide {
            1.0 - crate::render::easing::quad_out((elapsed / 1.0).clamp(0.0, 1.0))
        } else if let Some((elapsed, _, _, _)) = self.board_pop_elapsed {
            crate::render::easing::quad_out(((elapsed - 1.0) / 1.0).clamp(0.0, 1.0))
        } else {
            1.0
        }
    }

    fn render_match(&mut self, canvas: &mut Canvas, paused: bool) {
        let board_anim = self.current_board_anim();
        let preview_alpha = self.match_preview_alpha();
        let Some(sim) = self.match_sim.as_ref() else {
            return;
        };
        let play_layout =
            BoardLayout::compute(sim.level.cols, sim.level.rows, hud::match_play_area());
        let hover = if self.modals.is_empty()
            && !paused
            && !self.is_board_busy()
            && sim.is_ready
            && sim.is_started
        {
            play_layout.hit_test(self.mouse_pos.0, self.mouse_pos.1)
        } else {
            None
        };
        let swap_elig = sim
            .selected
            .map(|s| sim.get_swap_eligible(s))
            .unwrap_or_default();
        let color_blind = self
            .options_board_color_blind
            .unwrap_or(self.save.config.color_blind);

        self.renderer.draw_board(
            canvas,
            &sim.play_tiles,
            play_layout,
            sim.selected,
            &swap_elig,
            hover,
            sim.select_age,
            &[],
            color_blind,
            false,
            board_anim,
            self.board_age,
        );

        if !paused && preview_alpha > 0.004 {
            let goal_layout =
                BoardLayout::compute(sim.level.cols, sim.level.rows, hud::match_preview_area());
            self.renderer.draw_board(
                canvas,
                &sim.goal_tiles,
                goal_layout,
                None,
                &[],
                None,
                0.0,
                &[],
                color_blind,
                sim.mirror_mode,
                BoardAnim::Preview {
                    alpha: preview_alpha,
                },
                self.board_age,
            );
        }
        if self.hud_anim.overlay_alpha > 0.004 || (!paused && preview_alpha > 0.004) {
            hud::render_match_hud(
                canvas,
                &self.renderer.bank,
                &self.renderer.fonts,
                sim,
                self.hud_score_roller.value(),
                self.mouse_pos,
                self.is_board_busy(),
                preview_alpha,
                &self.hud_anim,
                !paused,
            );
        }
    }
}
