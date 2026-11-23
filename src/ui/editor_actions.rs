// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::audio::music_for_background;
use crate::game::Difficulty;
use crate::game::solver::{SolveOutcome, solve_level, validate_level_structure};
use crate::game::types::GameColor;
use crate::render::BoardLayout;
use crate::ui::editor::{
    ALL_GAME_COLORS, EDITOR_BACKGROUNDS, EditorState, EditorTool, LevelPropsDraft, PipeBehavior,
    PipeColorSlot, SolveStatus, editor_board_area,
};
use crate::ui::widgets::{ModalState, QuestionId, TextPromptPurpose, UiAction};
use crate::ui::{GameApp, PlayMode, Screen};
use std::time::Duration;

fn cycle_in<T: PartialEq + Copy>(list: &[T], cur: T) -> T {
    let pos = list.iter().position(|&v| v == cur).unwrap_or(0);
    list[(pos + 1) % list.len()]
}

fn cycle_str(list: &[&str], cur: &str) -> String {
    let pos = list
        .iter()
        .position(|s| s.eq_ignore_ascii_case(cur))
        .unwrap_or(0);
    list[(pos + 1) % list.len()].to_string()
}

impl GameApp {
    pub fn open_editor_for_selected_level(&mut self) {
        let Some(pack) = self
            .custom_packs
            .packs
            .get(self.custom_hub.selected_pack)
            .cloned()
        else {
            return;
        };
        if !pack.writable {
            self.show_status_toast("This pack is read-only. Use 'copy edit' first.");
            return;
        }
        let lvl_idx = self.custom_hub.selected_level;
        let Some(lvl) = pack.pack.levels.get(lvl_idx).cloned() else {
            return;
        };

        let bg = lvl.background_id.clone();
        self.bg_anim.load(&bg);
        self.bg_anim.lighten(0.0);
        self.audio.clear_delayed();
        self.audio.stop_music();
        self.audio.play_music(music_for_background(&bg), true);

        self.editor = Some(EditorState::new(
            pack.id,
            lvl_idx,
            lvl,
            self.custom_hub.play_difficulty,
        ));
        self.drag_start_tile = None;
        self.hovered_tile = None;
        self.screen = Screen::Editor;
    }

    pub fn return_to_editor(&mut self) {
        self.campaign_sim = None;
        self.match_sim = None;
        self.summary_state = None;
        self.champion_state = None;
        self.board_pop_elapsed = None;
        self.board_hide = None;
        self.info_banner = None;
        self.leaks.clear();
        self.fireworks.clear();
        self.blurbs.clear();
        self.play_mode = PlayMode::Campaign;

        let Some(bg) = self.editor.as_ref().map(|e| e.draft.background_id.clone()) else {
            self.open_custom_hub(false);
            return;
        };
        self.bg_anim.load(&bg);
        self.bg_anim.lighten(0.0);
        self.audio.clear_delayed();
        self.audio.stop_music();
        self.audio.play_music(music_for_background(&bg), true);
        self.screen = Screen::Editor;
    }

    pub fn close_editor_to_hub(&mut self) {
        let restore = self.editor.take().map(|e| (e.pack_id, e.level_idx));
        self.open_custom_hub(self.custom_hub.play_focused);
        if let Some((pack_id, lvl_idx)) = restore {
            self.select_pack_by_id(&pack_id);
            self.custom_hub.selected_level = lvl_idx;
            self.custom_hub.clamp_selection(&self.custom_packs);
            self.custom_hub.sync_pages_to_selection();
        }
    }

    pub fn handle_editor_action(&mut self, action: UiAction) {
        match action {
            UiAction::EditorSelectTool(tool) => {
                if let Some(ed) = &mut self.editor {
                    ed.tool = tool;
                }
            }
            UiAction::EditorRotateBrushCw => {
                if let Some(ed) = &mut self.editor {
                    ed.brush_dir = (ed.brush_dir + 1) % 4;
                }
            }
            UiAction::EditorSetBehavior(beh) => {
                if let Some(ed) = &mut self.editor {
                    ed.behavior = beh;
                    if beh != PipeBehavior::Fixed {
                        ed.mod_gate = false;
                        ed.mod_magic = false;
                    }
                }
            }
            UiAction::EditorToggleFreeze => {
                if let Some(ed) = &mut self.editor {
                    ed.mod_freeze = !ed.mod_freeze;
                    if ed.mod_freeze {
                        ed.mod_magic = false;
                    }
                }
            }
            UiAction::EditorToggleGate => {
                if let Some(ed) = &mut self.editor {
                    ed.mod_gate = !ed.mod_gate;
                    if ed.mod_gate {
                        ed.behavior = PipeBehavior::Fixed;
                        ed.mod_magic = false;
                    }
                }
            }
            UiAction::EditorToggleMagic => {
                if let Some(ed) = &mut self.editor {
                    ed.mod_magic = !ed.mod_magic;
                    if ed.mod_magic {
                        ed.behavior = PipeBehavior::Fixed;
                        ed.mod_gate = false;
                        ed.mod_freeze = false;
                    }
                }
            }
            UiAction::EditorSetPortalColor(col) => {
                if let Some(ed) = &mut self.editor {
                    ed.portal_color = col;
                    ed.tool = EditorTool::Portal;
                }
            }
            UiAction::EditorResizeCols(delta) => {
                if let Some(ed) = &mut self.editor {
                    let next = (ed.draft.cols as isize + delta as isize).max(1) as usize;
                    ed.resize_grid(next, ed.draft.rows);
                }
            }
            UiAction::EditorResizeRows(delta) => {
                if let Some(ed) = &mut self.editor {
                    let next = (ed.draft.rows as isize + delta as isize).max(1) as usize;
                    ed.resize_grid(ed.draft.cols, next);
                }
            }
            UiAction::EditorUndo => {
                if let Some(ed) = &mut self.editor
                    && ed.undo()
                {
                    self.audio.play_sfx("SwapTwoSound");
                }
            }
            UiAction::EditorRedo => {
                if let Some(ed) = &mut self.editor
                    && ed.redo()
                {
                    self.audio.play_sfx("SwapTwoSound");
                }
            }
            UiAction::EditorClearBoard => {
                self.modals.push(ModalState::Question {
                    id: QuestionId::ConfirmClearEditorBoard,
                    message: "Reset the grid to a blank starter layout?".to_string(),
                    yes_label: "yes",
                    no_label: Some("no"),
                });
            }
            UiAction::EditorVerify => self.run_editor_verify(),
            UiAction::EditorAutoPar => self.run_editor_auto_par(),
            UiAction::EditorToggleSolutionPath => {
                if let Some(ed) = &mut self.editor
                    && matches!(ed.solve_status, SolveStatus::Solvable(_))
                {
                    ed.show_solution_path = !ed.show_solution_path;
                }
            }
            UiAction::EditorOpenProps => {
                if let Some(ed) = &self.editor {
                    self.modals
                        .push(ModalState::LevelProps(LevelPropsDraft::from_level(
                            &ed.draft,
                            ed.test_difficulty,
                        )));
                }
            }
            UiAction::EditorTest => self.launch_editor_playtest(),
            UiAction::EditorSave => self.save_editor_level(),
            UiAction::EditorBack => {
                if self.editor.as_ref().is_some_and(|e| e.dirty) {
                    self.modals.push(ModalState::Question {
                        id: QuestionId::ConfirmDiscardEditor,
                        message: "Discard unsaved changes to this level?".to_string(),
                        yes_label: "yes",
                        no_label: Some("no"),
                    });
                } else {
                    self.close_editor_to_hub();
                }
            }
            UiAction::PropsEditTitle
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
            | UiAction::PropsApply => self.handle_level_props_action(action),
            _ => {}
        }
    }

    fn run_editor_verify(&mut self) {
        let Some(draft) = self.editor.as_ref().map(|e| e.draft.clone()) else {
            return;
        };
        if let Some(ed) = &mut self.editor {
            ed.refresh_validation();
        }
        let outcome = solve_level(&draft, Duration::from_millis(400));
        match outcome {
            SolveOutcome::Solvable(res) => {
                let msg = format!(
                    "Verified solvable! ({}m, {}p)",
                    res.moves.len(),
                    res.pipes_filled
                );
                if let Some(ed) = &mut self.editor {
                    ed.solve_status = SolveStatus::Solvable(res);
                    ed.show_solution_path = true;
                }
                self.audio.play_sfx("SwishSound");
                self.show_status_toast(&msg);
            }
            SolveOutcome::Unsolvable(msg) => {
                if let Some(ed) = &mut self.editor {
                    ed.solve_status = SolveStatus::Unsolvable(msg.clone());
                    ed.show_solution_path = false;
                }
                self.audio.play_sfx("AlarmBeepSound");
                self.show_status_toast(&msg);
            }
            SolveOutcome::BudgetExceeded => {
                if let Some(ed) = &mut self.editor {
                    ed.solve_status = SolveStatus::BudgetExceeded;
                    ed.show_solution_path = false;
                }
                self.show_status_toast("Solver timed out — use Test to verify.");
            }
        }
    }

    fn run_editor_auto_par(&mut self) {
        let cached = self.editor.as_ref().and_then(|e| match &e.solve_status {
            SolveStatus::Solvable(res) => Some(res.clone()),
            _ => None,
        });
        let res = match cached {
            Some(r) => r,
            None => {
                let Some(draft) = self.editor.as_ref().map(|e| e.draft.clone()) else {
                    return;
                };
                match solve_level(&draft, Duration::from_millis(400)) {
                    SolveOutcome::Solvable(r) => r,
                    SolveOutcome::Unsolvable(msg) => {
                        if let Some(ed) = &mut self.editor {
                            ed.solve_status = SolveStatus::Unsolvable(msg.clone());
                            ed.show_solution_path = false;
                        }
                        self.audio.play_sfx("AlarmBeepSound");
                        self.show_status_toast(&msg);
                        return;
                    }
                    SolveOutcome::BudgetExceeded => {
                        if let Some(ed) = &mut self.editor {
                            ed.solve_status = SolveStatus::BudgetExceeded;
                            ed.show_solution_path = false;
                        }
                        self.audio.play_sfx("AlarmBeepSound");
                        self.show_status_toast("Auto-Par failed: solver timed out.");
                        return;
                    }
                }
            }
        };

        let toast = format!(
            "Applied Auto-Par: {}m, {}p, {}:{:02}",
            res.suggested_par_moves,
            res.suggested_par_pipes,
            res.suggested_par_time_ms / 60000,
            (res.suggested_par_time_ms / 1000) % 60
        );
        if let Some(ed) = &mut self.editor {
            let keep_overlay = ed.show_solution_path;
            ed.push_undo();
            ed.draft.par_moves = res.suggested_par_moves;
            ed.draft.par_pipes = res.suggested_par_pipes;
            ed.draft.par_time_ms = res.suggested_par_time_ms;
            ed.solve_status = SolveStatus::Solvable(res);
            ed.show_solution_path = keep_overlay;
        }
        self.audio.play_sfx("BonusTwoSound");
        self.show_status_toast(&toast);
    }

    fn launch_editor_playtest(&mut self) {
        let Some((def, diff)) = self
            .editor
            .as_ref()
            .map(|e| (e.draft.clone(), e.test_difficulty))
        else {
            return;
        };
        let issues = validate_level_structure(&def);
        if let Some(first) = issues.first() {
            if let Some(ed) = &mut self.editor {
                ed.validation_issues = issues.clone();
            }
            self.audio.play_sfx("AlarmBeepSound");
            self.show_status_toast(&first.message);
            return;
        }
        self.fail_count = 0;
        self.temp_easy_revert = None;
        self.retry_backup = None;
        self.is_retrying_level = false;
        self.play_mode = PlayMode::EditorTest { difficulty: diff };
        self.launch_level_def(def);
    }

    fn save_editor_level(&mut self) {
        let Some((pack_id, level_idx, draft)) = self
            .editor
            .as_ref()
            .map(|e| (e.pack_id.clone(), e.level_idx, e.draft.clone()))
        else {
            return;
        };
        let Some(entry) = self
            .custom_packs
            .packs
            .iter()
            .find(|p| p.id == pack_id)
            .cloned()
        else {
            self.show_status_toast("Target pack no longer exists.");
            return;
        };
        if !entry.writable {
            self.show_status_toast("Target pack is read-only.");
            return;
        }
        let mut pack = entry.pack;
        if level_idx >= pack.levels.len() {
            self.show_status_toast("Target level index out of bounds.");
            return;
        }
        pack.levels[level_idx] = draft;
        for (i, l) in pack.levels.iter_mut().enumerate() {
            l.index = i;
        }
        match self.custom_packs.save_pack(&pack_id, pack) {
            Ok(_) => {
                if let Some(ed) = &mut self.editor {
                    ed.dirty = false;
                }
                self.select_pack_by_id(&pack_id);
                self.custom_hub.selected_level = level_idx;
                self.custom_hub.clamp_selection(&self.custom_packs);
                self.custom_hub.sync_pages_to_selection();
                self.audio.play_sfx("BonusTwoSound");
                self.show_status_toast("Saved level to pack!");
            }
            Err(e) => self.show_status_toast(&e),
        }
    }

    fn handle_level_props_action(&mut self, action: UiAction) {
        match action {
            UiAction::PropsEditTitle => {
                if let Some(ModalState::LevelProps(props)) = self.modals.last() {
                    let cur = props.title.clone();
                    self.modals.push(ModalState::TextPrompt {
                        purpose: TextPromptPurpose::EditLevelTitle,
                        prompt: "Enter level title:".to_string(),
                        input: cur,
                        max_len: 28,
                    });
                }
            }
            UiAction::PropsEditDesc => {
                if let Some(ModalState::LevelProps(props)) = self.modals.last() {
                    let cur = props.description.clone();
                    self.modals.push(ModalState::TextPrompt {
                        purpose: TextPromptPurpose::EditLevelDescription,
                        prompt: "Enter level subtitle:".to_string(),
                        input: cur,
                        max_len: 54,
                    });
                }
            }
            UiAction::PropsCycleBg => {
                if let Some(ModalState::LevelProps(props)) = self.modals.last_mut() {
                    props.background_id = cycle_str(EDITOR_BACKGROUNDS, &props.background_id);
                }
            }
            UiAction::PropsCycleLiquid => {
                if let Some(ModalState::LevelProps(props)) = self.modals.last_mut() {
                    props.liquid_color = cycle_in(ALL_GAME_COLORS, props.liquid_color);
                }
            }
            UiAction::PropsAdjustDelay(delta) => {
                if let Some(ModalState::LevelProps(props)) = self.modals.last_mut() {
                    props.start_delay = (props.start_delay + delta as f32).round().clamp(0.0, 60.0);
                }
            }
            UiAction::PropsAdjustSpeed(delta) => {
                if let Some(ModalState::LevelProps(props)) = self.modals.last_mut() {
                    let next = (props.liquid_speed + delta as f32 * 0.25).clamp(0.5, 6.0);
                    props.liquid_speed = (next * 4.0).round() * 0.25;
                }
            }
            UiAction::PropsCyclePipeColor(slot) => {
                if let Some(ModalState::LevelProps(props)) = self.modals.last_mut() {
                    let target: &mut GameColor = match slot {
                        PipeColorSlot::Mover => &mut props.mover_color,
                        PipeColorSlot::Rotater => &mut props.rotater_color,
                        PipeColorSlot::Stationary => &mut props.stationary_color,
                        PipeColorSlot::Gate => &mut props.gate_color,
                        PipeColorSlot::Freeze => &mut props.freeze_color,
                        PipeColorSlot::Magic => &mut props.magic_color,
                    };
                    *target = cycle_in(ALL_GAME_COLORS, *target);
                }
            }
            UiAction::PropsAutoPar => {
                let temp_lvl = self.editor.as_ref().and_then(|ed| {
                    if let Some(ModalState::LevelProps(props)) = self.modals.last() {
                        let mut l = ed.draft.clone();
                        props.apply_to(&mut l);
                        Some(l)
                    } else {
                        None
                    }
                });
                let Some(lvl) = temp_lvl else {
                    return;
                };
                match solve_level(&lvl, Duration::from_millis(400)) {
                    SolveOutcome::Solvable(res) => {
                        if let Some(ModalState::LevelProps(props)) = self.modals.last_mut() {
                            props.par_moves = res.suggested_par_moves;
                            props.par_pipes = res.suggested_par_pipes;
                            props.par_time_ms = res.suggested_par_time_ms;
                        }
                        self.audio.play_sfx("BonusTwoSound");
                        self.show_status_toast("Computed Auto-Par targets!");
                    }
                    SolveOutcome::Unsolvable(msg) => {
                        self.audio.play_sfx("AlarmBeepSound");
                        self.show_status_toast(&msg);
                    }
                    SolveOutcome::BudgetExceeded => {
                        self.audio.play_sfx("AlarmBeepSound");
                        self.show_status_toast("Solver timed out.");
                    }
                }
            }
            UiAction::PropsAdjustParMoves(delta) => {
                if let Some(ModalState::LevelProps(props)) = self.modals.last_mut() {
                    props.par_moves = (props.par_moves as i32 + delta).clamp(1, 99) as u32;
                }
            }
            UiAction::PropsAdjustParPipes(delta) => {
                if let Some(ModalState::LevelProps(props)) = self.modals.last_mut() {
                    props.par_pipes = (props.par_pipes as i32 + delta).clamp(1, 80) as u32;
                }
            }
            UiAction::PropsAdjustParTimeSec(delta_sec) => {
                if let Some(ModalState::LevelProps(props)) = self.modals.last_mut() {
                    let next = (props.par_time_ms as i32 + delta_sec * 1000).clamp(3000, 180_000);
                    props.par_time_ms = next as u32;
                }
            }
            UiAction::PropsCycleTestDiff => {
                if let Some(ModalState::LevelProps(props)) = self.modals.last_mut() {
                    props.test_difficulty = match props.test_difficulty {
                        Difficulty::Easy => Difficulty::Medium,
                        Difficulty::Medium => Difficulty::Hard,
                        Difficulty::Hard => Difficulty::Easy,
                    };
                }
            }
            UiAction::PropsApply => {
                let Some(ModalState::LevelProps(props)) = self.modals.pop() else {
                    return;
                };
                let mut bg_changed = None;
                if let Some(ed) = &mut self.editor {
                    let cached = match (&ed.solve_status, ed.show_solution_path) {
                        (SolveStatus::Solvable(r), show) => Some((r.clone(), show)),
                        _ => None,
                    };
                    if ed.draft.background_id != props.background_id {
                        bg_changed = Some(props.background_id.clone());
                    }
                    ed.push_undo();
                    props.apply_to(&mut ed.draft);
                    ed.test_difficulty = props.test_difficulty;
                    ed.refresh_validation();
                    if let Some((r, show)) = cached {
                        ed.solve_status = SolveStatus::Solvable(r);
                        ed.show_solution_path = show;
                    }
                }
                if let Some(bg) = bg_changed {
                    self.bg_anim.load(&bg);
                    self.bg_anim.lighten(0.0);
                    self.audio.stop_music();
                    self.audio.play_music(music_for_background(&bg), true);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn editor_mouse_down(&mut self, x: f32, y: f32) {
        let Some(ed) = &mut self.editor else {
            return;
        };
        let layout = BoardLayout::compute(ed.draft.cols, ed.draft.rows, editor_board_area());
        let Some((gx, gy)) = layout.hit_test(x, y) else {
            return;
        };
        if ed.tool == EditorTool::Rotate {
            if ed.rotate_cell(gx, gy, 1) {
                self.audio.play_sfx("TwistPieceSound");
            }
        } else {
            ed.is_painting = true;
            ed.stroke_undo_pushed = false;
            ed.last_painted_cell = Some((gx, gy));
            if ed.paint_cell(gx, gy, true) {
                ed.stroke_undo_pushed = true;
                self.audio.play_sfx("PopTwoSound");
            }
        }
    }

    pub(crate) fn editor_mouse_move(&mut self, x: f32, y: f32) {
        let Some(ed) = &mut self.editor else {
            return;
        };
        if !ed.is_painting || !ed.tool.supports_drag_paint() {
            return;
        }
        let layout = BoardLayout::compute(ed.draft.cols, ed.draft.rows, editor_board_area());
        if let Some(cell) = layout.hit_test(x, y)
            && ed.last_painted_cell != Some(cell)
        {
            ed.last_painted_cell = Some(cell);
            let need_undo = !ed.stroke_undo_pushed;
            if ed.paint_cell(cell.0, cell.1, need_undo) {
                ed.stroke_undo_pushed = true;
                self.audio.play_sfx("PopTwoSound");
            }
        }
    }

    pub(crate) fn editor_mouse_up(&mut self) {
        if let Some(ed) = &mut self.editor {
            ed.is_painting = false;
            ed.stroke_undo_pushed = false;
            ed.last_painted_cell = None;
        }
    }

    pub fn on_right_click(&mut self, x: f32, y: f32) {
        if self.screen != Screen::Editor || !self.modals.is_empty() {
            return;
        }
        let Some(ed) = &mut self.editor else {
            return;
        };
        let layout = BoardLayout::compute(ed.draft.cols, ed.draft.rows, editor_board_area());
        if let Some((gx, gy)) = layout.hit_test(x, y)
            && ed.rotate_cell(gx, gy, 1)
        {
            self.audio.play_sfx("TwistPieceSound");
        }
    }

    pub fn on_shortcut(&mut self, ch: char, ctrl: bool, shift: bool) {
        if self.screen != Screen::Editor || !self.modals.is_empty() {
            return;
        }
        if ctrl {
            match ch {
                'z' | 'Z' => {
                    self.trigger_action(if shift {
                        UiAction::EditorRedo
                    } else {
                        UiAction::EditorUndo
                    });
                }
                'y' | 'Y' => self.trigger_action(UiAction::EditorRedo),
                's' | 'S' => self.trigger_action(UiAction::EditorSave),
                _ => {}
            }
            return;
        }
        match ch {
            '1'..='8' => {
                let idx = (ch as usize) - ('1' as usize);
                if let Some(&tool) = EditorTool::ALL.get(idx) {
                    self.trigger_action(UiAction::EditorSelectTool(tool));
                }
            }
            'r' | 'R' => {
                if let Some(ed) = &mut self.editor {
                    let delta = if shift { 3 } else { 1 };
                    ed.brush_dir = (ed.brush_dir + delta) % 4;
                    self.audio.play_sfx("ClickOneSound");
                }
            }
            'q' | 'Q' => {
                if let Some(ed) = &mut self.editor {
                    ed.brush_dir = (ed.brush_dir + 3) % 4;
                    self.audio.play_sfx("ClickOneSound");
                }
            }
            'm' | 'M' => self.trigger_action(UiAction::EditorSetBehavior(PipeBehavior::Mover)),
            'o' | 'O' => self.trigger_action(UiAction::EditorSetBehavior(PipeBehavior::Rotater)),
            'f' | 'F' => self.trigger_action(UiAction::EditorSetBehavior(PipeBehavior::Fixed)),
            'v' | 'V' => self.trigger_action(UiAction::EditorVerify),
            't' | 'T' => self.trigger_action(UiAction::EditorTest),
            'p' | 'P' => self.trigger_action(UiAction::EditorOpenProps),
            _ => {}
        }
    }
}
