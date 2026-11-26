// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::render::{BoardLayout, CursorKind, Rect};
use crate::ui::editor::{EditorTool, build_editor_buttons, editor_board_area};
use crate::ui::hud;
use crate::ui::modals::{OPTIONS_FX_Y, OPTIONS_MUSIC_Y, OPTIONS_SLIDER_X};
use crate::ui::title::TitleSubmenu;
use crate::ui::widgets::{ModalState, QuestionId, UiAction};
use crate::ui::{DragSlider, GameApp, Screen};

impl GameApp {
    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        self.mouse_pos = (x, y);
        if let Some(sl) = self.dragging_slider {
            let v = ((x - OPTIONS_SLIDER_X) / 120.0).clamp(0.0, 1.0);
            match sl {
                DragSlider::Music => self.trigger_action(UiAction::SetMusicVol(v)),
                DragSlider::Sfx => self.trigger_action(UiAction::SetSfxVol(v)),
            }
        }
        if self.screen == Screen::Editor && self.modals.is_empty() {
            self.editor_mouse_move(x, y);
        }
        self.update_hover_sounds(x, y);
    }

    fn update_hover_sounds(&mut self, x: f32, y: f32) {
        let mut new_btn: Option<(&'static str, Option<&'static str>)> = None;
        for btn in self.current_buttons() {
            if btn.enabled && btn.rect.contains(x, y) {
                new_btn = Some((btn.id, btn.hover_sfx));
                break;
            }
        }
        let next_key = new_btn.map(|b| b.0);
        if next_key != self.hovered_button_id {
            self.hovered_button_id = next_key;
            if let Some((_, Some(sfx))) = new_btn {
                self.audio.play_sfx(sfx);
            }
        }

        if self.modals.is_empty() && !self.is_board_busy() {
            let next_tile = if self.screen == Screen::Playing {
                self.campaign_sim.as_ref().and_then(|sim| {
                    let l = BoardLayout::compute(
                        sim.level.cols,
                        sim.level.rows,
                        hud::campaign_board_area(),
                    );
                    l.hit_test(x, y)
                })
            } else if self.screen == Screen::MatchPlaying {
                self.match_sim.as_ref().and_then(|sim| {
                    let l = BoardLayout::compute(
                        sim.level.cols,
                        sim.level.rows,
                        hud::match_play_area(),
                    );
                    l.hit_test(x, y)
                })
            } else {
                None
            };
            if next_tile != self.hovered_tile {
                self.hovered_tile = next_tile;
                if let Some((tx, ty)) = next_tile {
                    let is_mover = if self.screen == Screen::Playing {
                        self.campaign_sim.as_ref().is_some_and(|s| {
                            s.tiles[ty][tx].is_moveable && !s.tiles[ty][tx].is_acting()
                        })
                    } else {
                        self.match_sim.as_ref().is_some_and(|s| {
                            s.play_tiles[ty][tx].is_moveable && !s.play_tiles[ty][tx].is_acting()
                        })
                    };
                    if is_mover {
                        self.audio.play_sfx("ClickOneSound");
                    }
                }
            }
        } else {
            self.hovered_tile = None;
        }
    }

    pub fn on_mouse_down(&mut self, x: f32, y: f32) {
        self.mouse_pos = (x, y);
        if matches!(self.modals.last(), Some(ModalState::Options { .. })) {
            if Rect::new(OPTIONS_SLIDER_X - 10.0, OPTIONS_FX_Y - 4.0, 140.0, 28.0).contains(x, y) {
                self.dragging_slider = Some(DragSlider::Sfx);
                let v = ((x - OPTIONS_SLIDER_X) / 120.0).clamp(0.0, 1.0);
                self.trigger_action(UiAction::SetSfxVol(v));
                return;
            }
            if Rect::new(OPTIONS_SLIDER_X - 10.0, OPTIONS_MUSIC_Y - 4.0, 140.0, 28.0).contains(x, y)
            {
                self.dragging_slider = Some(DragSlider::Music);
                let v = ((x - OPTIONS_SLIDER_X) / 120.0).clamp(0.0, 1.0);
                self.trigger_action(UiAction::SetMusicVol(v));
                return;
            }
        }

        for btn in self.current_buttons() {
            if btn.enabled && btn.rect.contains(x, y) {
                self.drag_start_tile = None;
                self.trigger_action(btn.action);
                return;
            }
        }

        if !self.modals.is_empty() || self.is_board_busy() {
            return;
        }

        if self.screen == Screen::Editor {
            self.editor_mouse_down(x, y);
            return;
        }

        if self.screen == Screen::Playing {
            if let Some(sim) = &mut self.campaign_sim {
                if let Some(tut) = sim.active_tutorial()
                    && tut.click_to_continue()
                {
                    let evs = sim.next_tutorial();
                    let l = BoardLayout::compute(
                        sim.level.cols,
                        sim.level.rows,
                        hud::campaign_board_area(),
                    );
                    self.handle_sim_events(evs, l);
                    return;
                }
                let layout = BoardLayout::compute(
                    sim.level.cols,
                    sim.level.rows,
                    hud::campaign_board_area(),
                );
                self.drag_start_tile = layout.hit_test(x, y);
            }
        } else if self.screen == Screen::MatchPlaying
            && let Some(sim) = &self.match_sim
        {
            let layout =
                BoardLayout::compute(sim.level.cols, sim.level.rows, hud::match_play_area());
            self.drag_start_tile = layout.hit_test(x, y);
        }
    }

    pub fn on_mouse_up(&mut self, x: f32, y: f32) {
        self.mouse_pos = (x, y);
        self.dragging_slider = None;
        if self.screen == Screen::Editor {
            self.editor_mouse_up();
        }
        let Some(start) = self.drag_start_tile.take() else {
            return;
        };
        if !self.modals.is_empty() || self.is_board_busy() {
            return;
        }
        if self.screen == Screen::Playing {
            let Some(sim) = &mut self.campaign_sim else {
                return;
            };
            let layout =
                BoardLayout::compute(sim.level.cols, sim.level.rows, hud::campaign_board_area());
            if let Some(end) = layout.hit_test(x, y) {
                let ev = if end != start {
                    sim.try_swap_drag(start, end)
                } else {
                    sim.click_tile(end.0, end.1)
                };
                self.handle_sim_events(ev, layout);
            }
        } else if self.screen == Screen::MatchPlaying {
            let Some(sim) = &mut self.match_sim else {
                return;
            };
            let layout =
                BoardLayout::compute(sim.level.cols, sim.level.rows, hud::match_play_area());
            if let Some(end) = layout.hit_test(x, y) {
                let ev = if end != start {
                    sim.try_swap_drag(start, end)
                } else {
                    sim.click_tile(end.0, end.1)
                };
                self.handle_sim_events(ev, layout);
            }
        }
    }

    pub fn has_text_modal(&self) -> bool {
        matches!(
            self.modals.last(),
            Some(ModalState::EnterName { .. } | ModalState::TextPrompt { .. })
        )
    }

    pub fn on_text_char(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }
        match self.modals.last_mut() {
            Some(ModalState::EnterName { input, .. }) if input.chars().count() < 32 => {
                input.push(ch);
            }
            Some(ModalState::TextPrompt { input, max_len, .. })
                if input.chars().count() < *max_len =>
            {
                input.push(ch);
            }
            _ => {}
        }
    }

    pub fn on_key_backspace(&mut self) {
        if let Some(ModalState::EnterName { input, .. } | ModalState::TextPrompt { input, .. }) =
            self.modals.last_mut()
        {
            input.pop();
        }
    }

    pub fn on_key_enter(&mut self) {
        match self.modals.last() {
            Some(ModalState::EnterName { .. }) => self.trigger_action(UiAction::SubmitName),
            Some(ModalState::TextPrompt { .. }) => self.trigger_action(UiAction::SubmitTextPrompt),
            Some(ModalState::EndlessSetup(cfg)) if cfg.any_enabled() => {
                self.trigger_action(UiAction::StartEndlessRun)
            }
            _ => {}
        }
    }

    pub fn on_key_escape(&mut self) {
        if let Some(ModalState::EnterName { allow_cancel, .. }) = self.modals.last() {
            if *allow_cancel {
                self.modals.pop();
            }
            return;
        }
        if let Some(top) = self.modals.last() {
            match top {
                ModalState::InGameDifficulty { .. }
                | ModalState::Award { .. }
                | ModalState::Question {
                    id: QuestionId::UnlockMatchNotice(_) | QuestionId::UnlockMirrorNotice(_),
                    ..
                } => return,
                _ => {
                    self.modals.pop();
                    return;
                }
            }
        }
        match self.screen {
            Screen::CustomHub => self.trigger_action(UiAction::HubBackToTitle),
            Screen::Editor => self.trigger_action(UiAction::EditorBack),
            Screen::Playing | Screen::MatchPlaying if self.can_pause() => {
                self.trigger_action(UiAction::PauseGame)
            }
            Screen::Paused => self.trigger_action(UiAction::ResumeGame),
            Screen::Title => {
                if self.title_submenu == TitleSubmenu::DifficultyChoices {
                    self.trigger_action(UiAction::BackToPlayMenu);
                } else if self.title_submenu == TitleSubmenu::PlayChoices {
                    self.trigger_action(UiAction::BackToMainMenu);
                }
            }
            _ => {}
        }
    }

    pub fn current_cursor(&self) -> CursorKind {
        if !self.modals.is_empty() || self.is_board_busy() {
            return CursorKind::Arrow;
        }
        let (mx, my) = self.mouse_pos;
        if self.screen == Screen::Editor {
            let Some(ed) = self.editor.as_ref() else {
                return CursorKind::Arrow;
            };
            for b in build_editor_buttons(ed) {
                if b.enabled && b.rect.contains(mx, my) {
                    return CursorKind::Arrow;
                }
            }
            let layout = BoardLayout::compute(ed.draft.cols, ed.draft.rows, editor_board_area());
            if layout.hit_test(mx, my).is_some() && ed.tool == EditorTool::Rotate {
                return CursorKind::Rotate;
            }
            return CursorKind::Arrow;
        }
        if self.screen == Screen::Playing {
            let Some(sim) = self.campaign_sim.as_ref() else {
                return CursorKind::Arrow;
            };
            if sim.active_tutorial().is_some_and(|t| t.click_to_continue()) {
                return CursorKind::Arrow;
            }
            if !sim.is_ready || (!sim.is_started && sim.active_tutorial().is_none()) {
                return CursorKind::Arrow;
            }
            for b in hud::build_playing_buttons(Some(sim), None, false) {
                if b.enabled && b.rect.contains(mx, my) {
                    return CursorKind::Arrow;
                }
            }
            let layout =
                BoardLayout::compute(sim.level.cols, sim.level.rows, hud::campaign_board_area());
            if let Some((tx, ty)) = layout.hit_test(mx, my) {
                let t = &sim.tiles[ty][tx];
                return if t.is_acting() {
                    CursorKind::Wait
                } else if t.is_moveable {
                    CursorKind::Arrow
                } else if t.can_rotate {
                    if sim.is_acting() {
                        CursorKind::Wait
                    } else {
                        CursorKind::Rotate
                    }
                } else {
                    CursorKind::Stop
                };
            }
        } else if self.screen == Screen::MatchPlaying {
            let Some(sim) = self.match_sim.as_ref() else {
                return CursorKind::Arrow;
            };
            if !sim.is_ready || !sim.is_started {
                return CursorKind::Arrow;
            }
            for b in hud::build_playing_buttons(None, Some(sim), false) {
                if b.enabled && b.rect.contains(mx, my) {
                    return CursorKind::Arrow;
                }
            }
            let layout =
                BoardLayout::compute(sim.level.cols, sim.level.rows, hud::match_play_area());
            if let Some((tx, ty)) = layout.hit_test(mx, my) {
                let t = &sim.play_tiles[ty][tx];
                return if t.is_acting() {
                    CursorKind::Wait
                } else if t.is_moveable {
                    CursorKind::Arrow
                } else if t.can_rotate {
                    if sim.is_acting() {
                        CursorKind::Wait
                    } else {
                        CursorKind::Rotate
                    }
                } else {
                    CursorKind::Stop
                };
            }
        }
        CursorKind::Arrow
    }
}
