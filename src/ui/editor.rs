// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::Difficulty;
use crate::game::level_io::{
    MAX_GRID_COLS, MAX_GRID_ROWS, MAX_LIQUID_SPEED, MAX_START_DELAY, MIN_GRID_DIM, MIN_LIQUID_SPEED,
};
use crate::game::solver::{SolveResult, ValidationIssue, validate_level_structure};
use crate::game::tile::build_tile_grid;
use crate::game::types::{GameColor, LevelDef, PieceShape, PieceType, Side, TileDef};
use crate::render::{
    BoardAnim, BoardLayout, Canvas, FontFamily, FontSet, LOGICAL_W, Rect, Renderer, TexBank,
    TextAlign,
};
use crate::ui::widgets::{
    UiAction, UiButton, draw_checkbox, draw_modal_bubble, draw_modal_button, draw_radio,
    draw_text_button,
};

pub const MAX_UNDO: usize = 64;
pub const EDITOR_BACKGROUNDS: &[&str] = &["NightHills", "DaySky", "SnowyHills", "OuterSpace"];
pub const ALL_GAME_COLORS: &[GameColor] = &[
    GameColor::Blue,
    GameColor::Green,
    GameColor::Red,
    GameColor::Orange,
    GameColor::Yellow,
    GameColor::Violet,
    GameColor::White,
    GameColor::Grey,
];
pub const PORTAL_PALETTE_COLORS: &[GameColor] = &[
    GameColor::Orange,
    GameColor::Violet,
    GameColor::Red,
    GameColor::Yellow,
    GameColor::Green,
    GameColor::Blue,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorTool {
    Elbow,
    Straight,
    Cross,
    Blocker,
    Source,
    Drain,
    Portal,
    Rotate,
}

impl EditorTool {
    pub const ALL: [EditorTool; 8] = [
        EditorTool::Elbow,
        EditorTool::Straight,
        EditorTool::Cross,
        EditorTool::Blocker,
        EditorTool::Source,
        EditorTool::Drain,
        EditorTool::Portal,
        EditorTool::Rotate,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Elbow => "Elbow",
            Self::Straight => "Straight",
            Self::Cross => "Cross",
            Self::Blocker => "Block",
            Self::Source => "Source",
            Self::Drain => "Drain",
            Self::Portal => "Portal",
            Self::Rotate => "Rotate",
        }
    }

    pub fn is_pipe(self) -> bool {
        matches!(self, Self::Elbow | Self::Straight | Self::Cross)
    }

    pub fn supports_drag_paint(self) -> bool {
        matches!(
            self,
            Self::Elbow | Self::Straight | Self::Cross | Self::Blocker
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeBehavior {
    Mover,
    Rotater,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeColorSlot {
    Mover,
    Rotater,
    Stationary,
    Gate,
    Freeze,
    Magic,
}

#[derive(Debug, Clone)]
pub enum SolveStatus {
    Unchecked,
    Solvable(SolveResult),
    Unsolvable(String),
    BudgetExceeded,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LevelPropsDraft {
    pub title: String,
    pub description: String,
    pub background_id: String,
    pub liquid_color: GameColor,
    pub start_delay: f32,
    pub liquid_speed: f32,
    pub mover_color: GameColor,
    pub rotater_color: GameColor,
    pub stationary_color: GameColor,
    pub gate_color: GameColor,
    pub freeze_color: GameColor,
    pub magic_color: GameColor,
    pub par_moves: u32,
    pub par_pipes: u32,
    pub par_time_ms: u32,
    pub test_difficulty: Difficulty,
}

impl LevelPropsDraft {
    pub fn from_level(lvl: &LevelDef, test_difficulty: Difficulty) -> Self {
        Self {
            title: lvl.title.clone(),
            description: lvl.description.clone(),
            background_id: lvl.background_id.clone(),
            liquid_color: lvl.liquid_color,
            start_delay: lvl.start_delay,
            liquid_speed: lvl.liquid_speed,
            mover_color: lvl.mover_color,
            rotater_color: lvl.rotater_color,
            stationary_color: lvl.stationary_color,
            gate_color: lvl.gate_color,
            freeze_color: lvl.freeze_color,
            magic_color: lvl.magic_color,
            par_moves: lvl.par_moves,
            par_pipes: lvl.par_pipes,
            par_time_ms: lvl.par_time_ms,
            test_difficulty,
        }
    }

    pub fn apply_to(&self, lvl: &mut LevelDef) {
        lvl.title = self.title.clone();
        lvl.description = self.description.clone();
        lvl.background_id = self.background_id.clone();
        lvl.liquid_color = self.liquid_color;
        lvl.start_delay = self.start_delay;
        lvl.liquid_speed = self.liquid_speed;
        lvl.mover_color = self.mover_color;
        lvl.rotater_color = self.rotater_color;
        lvl.stationary_color = self.stationary_color;
        lvl.gate_color = self.gate_color;
        lvl.freeze_color = self.freeze_color;
        lvl.magic_color = self.magic_color;
        lvl.par_moves = self.par_moves;
        lvl.par_pipes = self.par_pipes;
        lvl.par_time_ms = self.par_time_ms;
    }
}

#[derive(Debug, Clone)]
pub struct EditorState {
    pub pack_id: String,
    pub level_idx: usize,
    pub draft: LevelDef,
    pub dirty: bool,
    pub tool: EditorTool,
    pub brush_dir: u8,
    pub behavior: PipeBehavior,
    pub mod_freeze: bool,
    pub mod_gate: bool,
    pub mod_magic: bool,
    pub portal_color: GameColor,
    pub undo_stack: Vec<LevelDef>,
    pub redo_stack: Vec<LevelDef>,
    pub validation_issues: Vec<ValidationIssue>,
    pub solve_status: SolveStatus,
    pub show_solution_path: bool,
    pub is_painting: bool,
    pub stroke_undo_pushed: bool,
    pub last_painted_cell: Option<(usize, usize)>,
    pub test_difficulty: Difficulty,
}

impl EditorState {
    pub fn new(
        pack_id: String,
        level_idx: usize,
        draft: LevelDef,
        test_difficulty: Difficulty,
    ) -> Self {
        let mut st = Self {
            pack_id,
            level_idx,
            draft,
            dirty: false,
            tool: EditorTool::Elbow,
            brush_dir: 0,
            behavior: PipeBehavior::Mover,
            mod_freeze: false,
            mod_gate: false,
            mod_magic: false,
            portal_color: GameColor::Orange,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            validation_issues: Vec::new(),
            solve_status: SolveStatus::Unchecked,
            show_solution_path: false,
            is_painting: false,
            stroke_undo_pushed: false,
            last_painted_cell: None,
            test_difficulty,
        };
        st.normalize_tiles();
        st.refresh_validation();
        st
    }

    pub fn normalize_tiles(&mut self) {
        let cols = self.draft.cols.clamp(MIN_GRID_DIM, MAX_GRID_COLS);
        let rows = self.draft.rows.clamp(MIN_GRID_DIM, MAX_GRID_ROWS);
        self.draft.cols = cols;
        self.draft.rows = rows;

        let mut normalized = Vec::with_capacity(cols * rows);
        for y in 0..rows {
            for x in 0..cols {
                if let Some(existing) = self.draft.tiles.iter().find(|t| t.x == x && t.y == y) {
                    let mut t = existing.clone();
                    t.direction %= 4;
                    normalized.push(t);
                } else {
                    normalized.push(TileDef {
                        x,
                        y,
                        ..TileDef::dummy_blocker()
                    });
                }
            }
        }
        self.draft.tiles = normalized;
    }

    pub fn tile_index(&self, x: usize, y: usize) -> Option<usize> {
        if x < self.draft.cols && y < self.draft.rows {
            Some(y * self.draft.cols + x)
        } else {
            None
        }
    }

    pub fn refresh_validation(&mut self) {
        self.validation_issues = validate_level_structure(&self.draft);
    }

    pub fn invalidate_solve(&mut self) {
        self.solve_status = SolveStatus::Unchecked;
        self.show_solution_path = false;
    }

    pub fn push_undo(&mut self) {
        if self.undo_stack.len() >= MAX_UNDO {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(self.draft.clone());
        self.redo_stack.clear();
        self.dirty = true;
        self.invalidate_solve();
    }

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack.push(self.draft.clone());
        self.draft = prev;
        self.normalize_tiles();
        self.refresh_validation();
        self.invalidate_solve();
        self.dirty = true;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push(self.draft.clone());
        self.draft = next;
        self.normalize_tiles();
        self.refresh_validation();
        self.invalidate_solve();
        self.dirty = true;
        true
    }

    pub fn brush_tile_def(&self, x: usize, y: usize) -> Option<TileDef> {
        let dir = self.brush_dir % 4;
        let tile = match self.tool {
            EditorTool::Elbow | EditorTool::Straight | EditorTool::Cross => TileDef {
                x,
                y,
                shape: match self.tool {
                    EditorTool::Elbow => PieceShape::Elbow,
                    EditorTool::Straight => PieceShape::Straight,
                    _ => PieceShape::Cross,
                },
                piece_type: if self.mod_freeze {
                    PieceType::FrozenPipe
                } else {
                    PieceType::Pipe
                },
                direction: dir,
                is_moveable: self.behavior == PipeBehavior::Mover,
                can_rotate: self.behavior == PipeBehavior::Rotater,
                is_gate: self.mod_gate,
                is_freeze: self.mod_freeze,
                is_magic: self.mod_magic,
                secondary_color: None,
            },
            EditorTool::Blocker => TileDef {
                x,
                y,
                ..TileDef::dummy_blocker()
            },
            EditorTool::Source => TileDef {
                x,
                y,
                shape: PieceShape::Spigot,
                piece_type: PieceType::Source,
                direction: dir,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
            EditorTool::Drain => TileDef {
                x,
                y,
                shape: PieceShape::Spigot,
                piece_type: PieceType::Drain,
                direction: dir,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
            EditorTool::Portal => TileDef {
                x,
                y,
                shape: PieceShape::Spigot,
                piece_type: PieceType::Portal,
                direction: dir,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: Some(self.portal_color),
            },
            EditorTool::Rotate => return None,
        };
        Some(tile)
    }

    pub fn paint_cell(&mut self, x: usize, y: usize, push_undo_now: bool) -> bool {
        let Some(idx) = self.tile_index(x, y) else {
            return false;
        };
        let Some(new_tile) = self.brush_tile_def(x, y) else {
            return false;
        };
        let unique_kind = matches!(new_tile.piece_type, PieceType::Source | PieceType::Drain);
        let other_same = unique_kind
            && self
                .draft
                .tiles
                .iter()
                .enumerate()
                .any(|(i, t)| i != idx && t.piece_type == new_tile.piece_type);
        if self.draft.tiles[idx] == new_tile && !other_same {
            return false;
        }
        if push_undo_now {
            self.push_undo();
        }
        if unique_kind {
            for (i, t) in self.draft.tiles.iter_mut().enumerate() {
                if i != idx && t.piece_type == new_tile.piece_type {
                    *t = TileDef {
                        x: t.x,
                        y: t.y,
                        ..TileDef::dummy_blocker()
                    };
                }
            }
        }
        self.draft.tiles[idx] = new_tile;
        self.refresh_validation();
        self.invalidate_solve();
        self.dirty = true;
        true
    }

    pub fn rotate_cell(&mut self, x: usize, y: usize, delta_cw: u8) -> bool {
        let Some(idx) = self.tile_index(x, y) else {
            return false;
        };
        if self.draft.tiles[idx].piece_type == PieceType::Blocker {
            return false;
        }
        self.push_undo();
        self.draft.tiles[idx].direction = (self.draft.tiles[idx].direction + delta_cw) % 4;
        self.refresh_validation();
        true
    }

    pub fn resize_grid(&mut self, new_cols: usize, new_rows: usize) -> bool {
        let cols = new_cols.clamp(MIN_GRID_DIM, MAX_GRID_COLS);
        let rows = new_rows.clamp(MIN_GRID_DIM, MAX_GRID_ROWS);
        if cols == self.draft.cols && rows == self.draft.rows {
            return false;
        }
        self.push_undo();
        self.draft.cols = cols;
        self.draft.rows = rows;
        self.draft.tiles.retain(|t| t.x < cols && t.y < rows);
        self.normalize_tiles();
        self.refresh_validation();
        true
    }

    pub fn clear_to_blank(&mut self) {
        self.push_undo();
        let cols = self.draft.cols.clamp(MIN_GRID_DIM, MAX_GRID_COLS);
        let rows = self.draft.rows.clamp(MIN_GRID_DIM, MAX_GRID_ROWS);
        let mid_y = rows / 2;
        let mut tiles = Vec::with_capacity(cols * rows);
        for y in 0..rows {
            for x in 0..cols {
                let tile = if y == mid_y && x == 0 {
                    TileDef {
                        x,
                        y,
                        shape: PieceShape::Spigot,
                        piece_type: PieceType::Source,
                        direction: 3,
                        is_moveable: false,
                        can_rotate: false,
                        is_gate: false,
                        is_freeze: false,
                        is_magic: false,
                        secondary_color: None,
                    }
                } else if y == mid_y && x + 1 == cols {
                    TileDef {
                        x,
                        y,
                        shape: PieceShape::Spigot,
                        piece_type: PieceType::Drain,
                        direction: 1,
                        is_moveable: false,
                        can_rotate: false,
                        is_gate: false,
                        is_freeze: false,
                        is_magic: false,
                        secondary_color: None,
                    }
                } else if y == mid_y {
                    TileDef {
                        x,
                        y,
                        shape: PieceShape::Straight,
                        piece_type: PieceType::Pipe,
                        direction: 1,
                        is_moveable: false,
                        can_rotate: true,
                        is_gate: false,
                        is_freeze: false,
                        is_magic: false,
                        secondary_color: None,
                    }
                } else {
                    TileDef {
                        x,
                        y,
                        ..TileDef::dummy_blocker()
                    }
                };
                tiles.push(tile);
            }
        }
        self.draft.tiles = tiles;
        self.normalize_tiles();
        self.refresh_validation();
    }

    pub fn active_brush_color(&self) -> GameColor {
        if self.behavior == PipeBehavior::Rotater {
            self.draft.rotater_color
        } else if self.behavior == PipeBehavior::Mover {
            self.draft.mover_color
        } else if self.mod_gate {
            self.draft.gate_color
        } else if self.mod_freeze {
            self.draft.freeze_color
        } else if self.mod_magic {
            self.draft.magic_color
        } else {
            self.draft.stationary_color
        }
    }
}

pub fn editor_board_area() -> Rect {
    Rect::new(264.0, 52.0, 520.0, 470.0)
}

fn palette_slot_rect(idx: usize) -> Rect {
    let col = idx % 4;
    let row = idx / 4;
    Rect::new(
        29.0 + col as f32 * 50.0,
        58.0 + row as f32 * 50.0,
        46.0,
        46.0,
    )
}

pub fn build_editor_buttons(state: &EditorState) -> Vec<UiButton> {
    let mut out = Vec::new();
    for (i, tool) in EditorTool::ALL.iter().copied().enumerate() {
        out.push(UiButton {
            id: "ed_tool",
            rect: palette_slot_rect(i),
            action: UiAction::EditorSelectTool(tool),
            enabled: true,
            hover_sfx: None,
        });
    }

    let is_pipe = state.tool.is_pipe();
    out.extend([
        UiButton {
            id: "ed_rot_brush",
            rect: Rect::new(134.0, 160.0, 94.0, 22.0),
            action: UiAction::EditorRotateBrushCw,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "ed_mode_mover",
            rect: Rect::new(28.0, 212.0, 198.0, 20.0),
            action: UiAction::EditorSetBehavior(PipeBehavior::Mover),
            enabled: is_pipe,
            hover_sfx: None,
        },
        UiButton {
            id: "ed_mode_rotater",
            rect: Rect::new(28.0, 234.0, 198.0, 20.0),
            action: UiAction::EditorSetBehavior(PipeBehavior::Rotater),
            enabled: is_pipe,
            hover_sfx: None,
        },
        UiButton {
            id: "ed_mode_fixed",
            rect: Rect::new(28.0, 256.0, 198.0, 20.0),
            action: UiAction::EditorSetBehavior(PipeBehavior::Fixed),
            enabled: is_pipe,
            hover_sfx: None,
        },
        UiButton {
            id: "ed_mod_freeze",
            rect: Rect::new(28.0, 304.0, 198.0, 20.0),
            action: UiAction::EditorToggleFreeze,
            enabled: is_pipe,
            hover_sfx: None,
        },
        UiButton {
            id: "ed_mod_gate",
            rect: Rect::new(28.0, 326.0, 198.0, 20.0),
            action: UiAction::EditorToggleGate,
            enabled: is_pipe,
            hover_sfx: None,
        },
        UiButton {
            id: "ed_mod_magic",
            rect: Rect::new(28.0, 348.0, 198.0, 20.0),
            action: UiAction::EditorToggleMagic,
            enabled: is_pipe,
            hover_sfx: None,
        },
    ]);

    for (i, &col) in PORTAL_PALETTE_COLORS.iter().enumerate() {
        out.push(UiButton {
            id: "ed_portal_col",
            rect: Rect::new(29.0 + i as f32 * 33.0, 394.0, 28.0, 20.0),
            action: UiAction::EditorSetPortalColor(col),
            enabled: true,
            hover_sfx: None,
        });
    }

    let can_solve = state.validation_issues.is_empty();
    let has_solution = matches!(state.solve_status, SolveStatus::Solvable(_));
    out.extend([
        UiButton {
            id: "ed_cols_dec",
            rect: Rect::new(112.0, 444.0, 28.0, 22.0),
            action: UiAction::EditorResizeCols(-1),
            enabled: state.draft.cols > MIN_GRID_DIM,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "ed_cols_inc",
            rect: Rect::new(182.0, 444.0, 28.0, 22.0),
            action: UiAction::EditorResizeCols(1),
            enabled: state.draft.cols < MAX_GRID_COLS,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "ed_rows_dec",
            rect: Rect::new(112.0, 470.0, 28.0, 22.0),
            action: UiAction::EditorResizeRows(-1),
            enabled: state.draft.rows > MIN_GRID_DIM,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "ed_rows_inc",
            rect: Rect::new(182.0, 470.0, 28.0, 22.0),
            action: UiAction::EditorResizeRows(1),
            enabled: state.draft.rows < MAX_GRID_ROWS,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "ed_undo",
            rect: Rect::new(16.0, 538.0, 68.0, 22.0),
            action: UiAction::EditorUndo,
            enabled: !state.undo_stack.is_empty(),
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "ed_redo",
            rect: Rect::new(90.0, 538.0, 68.0, 22.0),
            action: UiAction::EditorRedo,
            enabled: !state.redo_stack.is_empty(),
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "ed_clear",
            rect: Rect::new(164.0, 538.0, 74.0, 22.0),
            action: UiAction::EditorClearBoard,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "ed_autopar",
            rect: Rect::new(16.0, 566.0, 108.0, 22.0),
            action: UiAction::EditorAutoPar,
            enabled: can_solve,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "ed_path",
            rect: Rect::new(132.0, 566.0, 128.0, 22.0),
            action: UiAction::EditorToggleSolutionPath,
            enabled: has_solution,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "ed_props",
            rect: Rect::new(284.0, 538.0, 94.0, 53.0),
            action: UiAction::EditorOpenProps,
            enabled: true,
            hover_sfx: None,
        },
        UiButton {
            id: "ed_verify",
            rect: Rect::new(386.0, 538.0, 94.0, 53.0),
            action: UiAction::EditorVerify,
            enabled: true,
            hover_sfx: None,
        },
        UiButton {
            id: "ed_test",
            rect: Rect::new(488.0, 538.0, 94.0, 53.0),
            action: UiAction::EditorTest,
            enabled: true,
            hover_sfx: None,
        },
        UiButton {
            id: "ed_save",
            rect: Rect::new(590.0, 538.0, 94.0, 53.0),
            action: UiAction::EditorSave,
            enabled: true,
            hover_sfx: None,
        },
        UiButton {
            id: "ed_back",
            rect: Rect::new(692.0, 538.0, 94.0, 53.0),
            action: UiAction::EditorBack,
            enabled: true,
            hover_sfx: None,
        },
    ]);

    out
}

#[allow(clippy::too_many_arguments)]
pub fn render_editor(
    canvas: &mut Canvas,
    renderer: &mut Renderer,
    state: &EditorState,
    color_blind: bool,
    time: f32,
    mouse_pos: (f32, f32),
    has_modal: bool,
) {
    let mpos = if has_modal {
        (-999.0, -999.0)
    } else {
        mouse_pos
    };
    let area = editor_board_area();
    let layout = BoardLayout::compute(state.draft.cols, state.draft.rows, area);

    // Top status banner above the board
    canvas.draw_rounded_rect(
        Rect::new(254.0, 10.0, 532.0, 32.0),
        8.0,
        (0x05, 0x0D, 0x2B, 200),
        Some((0x1C, 0x4A, 0x84, 230, 2.0)),
    );
    let dirty_mark = if state.dirty { "*" } else { "" };
    let header_title = format!(
        "{}. {}{}",
        state.level_idx + 1,
        truncate_chars(&state.draft.title, 18),
        dirty_mark
    );
    renderer.fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        266.0,
        15.0,
        16.0,
        (255, 255, 255),
        (0x05, 0x0D, 0x2B),
        TextAlign::Left,
        &header_title,
    );

    let (status_str, status_col) = editor_status_summary(state);
    renderer.fonts.draw(
        canvas,
        FontFamily::Neuropol,
        774.0,
        17.0,
        13.0,
        status_col,
        TextAlign::Right,
        &status_str,
    );

    // Board grid frame & cell guides
    let board_w = layout.tile_px * state.draft.cols as f32;
    let board_h = layout.tile_px * state.draft.rows as f32;
    canvas.draw_rounded_rect(
        Rect::new(layout.origin_x, layout.origin_y, board_w, board_h),
        6.0,
        (0x05, 0x0D, 0x2B, 96),
        Some((0x59, 0x95, 0xC1, 160, 2.0)),
    );
    for c in 1..state.draft.cols {
        let gx = layout.origin_x + c as f32 * layout.tile_px;
        canvas.draw_line(
            gx,
            layout.origin_y,
            gx,
            layout.origin_y + board_h,
            1.0,
            (0x59, 0x95, 0xC1, 70),
        );
    }
    for r in 1..state.draft.rows {
        let gy = layout.origin_y + r as f32 * layout.tile_px;
        canvas.draw_line(
            layout.origin_x,
            gy,
            layout.origin_x + board_w,
            gy,
            1.0,
            (0x59, 0x95, 0xC1, 70),
        );
    }

    let hover = if !has_modal {
        layout.hit_test(mpos.0, mpos.1)
    } else {
        None
    };
    let grid = build_tile_grid(&state.draft);
    renderer.draw_board(
        canvas,
        &grid,
        layout,
        None,
        &[],
        hover,
        0.0,
        &[],
        color_blind,
        false,
        BoardAnim::Active,
        time,
    );

    // Corner tags on Gate / Magic tiles so modifiers stay unmistakable with custom colors
    for row in &grid {
        for t in row {
            if !t.is_gate && !t.is_magic {
                continue;
            }
            let (cx, cy) = layout.tile_center(t.x, t.y);
            let tl_x = cx - layout.tile_px * 0.5 + 3.0;
            let tl_y = cy - layout.tile_px * 0.5 + 3.0;
            let tag = if t.is_gate && t.is_magic {
                "G+M"
            } else if t.is_gate {
                "G"
            } else {
                "M"
            };
            let tw = if tag.len() > 1 { 26.0 } else { 15.0 };
            canvas.draw_rounded_rect(
                Rect::new(tl_x, tl_y, tw, 14.0),
                3.0,
                (0x05, 0x0D, 0x2B, 215),
                Some((0xC9, 0xDA, 0x41, 200, 1.0)),
            );
            renderer.fonts.draw(
                canvas,
                FontFamily::Neuropol,
                tl_x + tw * 0.5,
                tl_y + 1.0,
                10.0,
                (0xC9, 0xDA, 0x41),
                TextAlign::Center,
                tag,
            );
        }
    }

    if state.show_solution_path
        && let SolveStatus::Solvable(ref res) = state.solve_status
    {
        draw_solution_overlay(canvas, &grid, layout, res, time);
    }

    // Highlight cells with structural validation problems
    let pulse = (140.0 + 95.0 * (time * 5.0).sin().abs()) as u8;
    for issue in &state.validation_issues {
        if let Some((ix, iy)) = issue.pos
            && ix < state.draft.cols
            && iy < state.draft.rows
        {
            let rx = layout.origin_x + ix as f32 * layout.tile_px + 2.0;
            let ry = layout.origin_y + iy as f32 * layout.tile_px + 2.0;
            canvas.draw_rounded_rect(
                Rect::new(rx, ry, layout.tile_px - 4.0, layout.tile_px - 4.0),
                4.0,
                (0xFF, 0x55, 0x48, 35),
                Some((0xFF, 0x55, 0x48, pulse, 2.5)),
            );
        }
    }

    if let Some((hx, hy)) = hover {
        if state.tool != EditorTool::Rotate {
            let (cx, cy) = layout.tile_center(hx, hy);
            draw_tool_icon(
                canvas,
                &renderer.bank,
                state.tool,
                cx,
                cy,
                layout.tile_px,
                (state.brush_dir % 4) as f32 * 90.0,
                state.active_brush_color(),
                state.draft.stationary_color,
                state.portal_color,
                state.mod_freeze,
                0.45,
            );
        }
        let rx = layout.origin_x + hx as f32 * layout.tile_px + 1.5;
        let ry = layout.origin_y + hy as f32 * layout.tile_px + 1.5;
        canvas.draw_rounded_rect(
            Rect::new(rx, ry, layout.tile_px - 3.0, layout.tile_px - 3.0),
            4.0,
            (0, 0, 0, 0),
            Some((0xC9, 0xDA, 0x41, 220, 2.0)),
        );
    }

    render_left_palette(canvas, &renderer.bank, &renderer.fonts, state, mpos);
    render_bottom_bar(canvas, &renderer.bank, &renderer.fonts, state, mpos);
}

fn editor_status_summary(state: &EditorState) -> (String, (u8, u8, u8)) {
    if let Some(first) = state.validation_issues.first() {
        return (truncate_chars(&first.message, 30), (0xFF, 0x8C, 0x80));
    }
    match &state.solve_status {
        SolveStatus::Unchecked => (
            format!(
                "Par {}m / {}p / {}s",
                state.draft.par_moves,
                state.draft.par_pipes,
                state.draft.par_time_ms / 1000
            ),
            (0xC0, 0xD4, 0xF0),
        ),
        SolveStatus::Solvable(res) => (
            format!(
                "Solvable: {}m, {}p (Par {}m)",
                res.moves.len(),
                res.pipes_filled,
                state.draft.par_moves
            ),
            (0x8E, 0xFF, 0x8A),
        ),
        SolveStatus::Unsolvable(msg) => (truncate_chars(msg, 30), (0xFF, 0x8C, 0x80)),
        SolveStatus::BudgetExceeded => (
            "Solver timed out (use Test)".to_string(),
            (0xFF, 0xDC, 0x73),
        ),
    }
}

fn side_edge_point(cx: f32, cy: f32, half: f32, side: Side) -> (f32, f32) {
    let (dx, dy) = side.delta();
    (cx + dx as f32 * half, cy + dy as f32 * half)
}

fn draw_solution_overlay(
    canvas: &mut Canvas,
    grid: &[Vec<crate::game::Tile>],
    layout: BoardLayout,
    res: &SolveResult,
    time: f32,
) {
    let half = layout.tile_px * 0.5;
    let pulse_a = (195.0 + 40.0 * (time * 3.0).sin()) as u8;
    for (idx, seg) in res.path.iter().enumerate() {
        let (cx, cy) = layout.tile_center(seg.x, seg.y);
        if seg.entry != Side::Center {
            let (ex, ey) = side_edge_point(cx, cy, half, seg.entry);
            canvas.draw_line(ex, ey, cx, cy, 10.0, (0x40, 0xFF, 0x60, 95));
            canvas.draw_line(ex, ey, cx, cy, 4.5, (0xA8, 0xFF, 0x78, pulse_a));
        }
        if seg.exit != Side::Center {
            let (ox, oy) = side_edge_point(cx, cy, half, seg.exit);
            canvas.draw_line(cx, cy, ox, oy, 10.0, (0x40, 0xFF, 0x60, 95));
            canvas.draw_line(cx, cy, ox, oy, 4.5, (0xA8, 0xFF, 0x78, pulse_a));
        } else if grid
            .get(seg.y)
            .and_then(|r| r.get(seg.x))
            .is_some_and(|t| t.piece_type == PieceType::Portal)
        {
            canvas.stroke_circle_alpha(cx, cy, half * 0.55, 3.0, (0xA8, 0xFF, 0x78, 210));
            if let Some(next) = res.path.get(idx + 1)
                && next.entry == Side::Center
            {
                let (nx, ny) = layout.tile_center(next.x, next.y);
                canvas.draw_line(cx, cy, nx, ny, 2.5, (0xC8, 0x90, 0xFF, 165));
                canvas.stroke_circle_alpha(nx, ny, half * 0.55, 3.0, (0xA8, 0xFF, 0x78, 210));
            }
        }
        canvas.fill_circle_alpha(cx, cy, 4.0, (0xE0, 0xFF, 0xA0, 230));
    }
}

fn render_left_palette(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    state: &EditorState,
    mouse_pos: (f32, f32),
) {
    draw_modal_bubble(canvas, Rect::new(14.0, 14.0, 226.0, 514.0));
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        127.0,
        24.0,
        20.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Center,
        "-Tools-",
    );

    let pipe_col = state.active_brush_color();
    let rot_deg = (state.brush_dir % 4) as f32 * 90.0;
    for (i, tool) in EditorTool::ALL.iter().copied().enumerate() {
        let r = palette_slot_rect(i);
        let selected = state.tool == tool;
        let hovered = r.contains(mouse_pos.0, mouse_pos.1);
        let fill = if selected {
            (0x59, 0x95, 0xC1, 220)
        } else if hovered {
            (0x59, 0x95, 0xC1, 105)
        } else {
            (0x05, 0x0D, 0x2B, 200)
        };
        let border = if selected {
            (0xC9, 0xDA, 0x41, 255, 2.5)
        } else {
            (0x0E, 0x15, 0x32, 255, 2.0)
        };
        canvas.draw_rounded_rect(r, 6.0, fill, Some(border));

        let cx = r.x + r.w * 0.5;
        let cy = r.y + r.h * 0.5;
        draw_tool_icon(
            canvas,
            bank,
            tool,
            cx,
            cy,
            34.0,
            rot_deg,
            pipe_col,
            state.draft.stationary_color,
            state.portal_color,
            state.mod_freeze,
            1.0,
        );
    }

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        28.0,
        162.0,
        14.0,
        (0xC0, 0xD4, 0xF0),
        TextAlign::Left,
        &format!("Tool: {}", state.tool.label()),
    );
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(134.0, 160.0, 94.0, 22.0),
        228.0,
        161.0,
        14.0,
        TextAlign::Right,
        &format!("[Rot {}°]", (state.brush_dir % 4) * 90),
        Some((0x42, 0x83, 0xF3)),
        true,
        mouse_pos,
    );

    // Pipe Mode radios
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        28.0,
        190.0,
        15.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Left,
        "Pipe Mode",
    );
    let is_pipe = state.tool.is_pipe();
    for (label, beh, col, y) in [
        ("Mover", PipeBehavior::Mover, state.draft.mover_color, 212.0),
        (
            "Rotater",
            PipeBehavior::Rotater,
            state.draft.rotater_color,
            234.0,
        ),
        (
            "Fixed",
            PipeBehavior::Fixed,
            state.draft.stationary_color,
            256.0,
        ),
    ] {
        let r = Rect::new(28.0, y, 198.0, 20.0);
        if is_pipe && r.contains(mouse_pos.0, mouse_pos.1) {
            canvas.fill_rect_alpha(r, 0x59, 0x95, 0xC1, 65);
        }
        draw_small_radio(
            canvas,
            fonts,
            30.0,
            y,
            label,
            state.behavior == beh,
            is_pipe,
        );
        draw_color_swatch(canvas, Rect::new(202.0, y + 3.0, 18.0, 14.0), col, false);
    }

    // Pipe Modifiers checkboxes
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        28.0,
        282.0,
        15.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Left,
        "Modifiers",
    );
    for (label, active, col, y) in [
        ("Frozen", state.mod_freeze, state.draft.freeze_color, 304.0),
        ("Gate", state.mod_gate, state.draft.gate_color, 326.0),
        ("Magic", state.mod_magic, state.draft.magic_color, 348.0),
    ] {
        let r = Rect::new(28.0, y, 198.0, 20.0);
        if is_pipe && r.contains(mouse_pos.0, mouse_pos.1) {
            canvas.fill_rect_alpha(r, 0x59, 0x95, 0xC1, 65);
        }
        draw_small_checkbox(canvas, fonts, 30.0, y + 2.0, label, active, is_pipe);
        draw_color_swatch(canvas, Rect::new(202.0, y + 3.0, 18.0, 14.0), col, false);
    }

    // Portal Color swatches
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        28.0,
        373.0,
        15.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Left,
        "Portal Color",
    );
    for (i, &col) in PORTAL_PALETTE_COLORS.iter().enumerate() {
        let r = Rect::new(29.0 + i as f32 * 33.0, 394.0, 28.0, 20.0);
        draw_color_swatch(canvas, r, col, state.portal_color == col);
    }

    // Grid Size steppers
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        28.0,
        422.0,
        15.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Left,
        "Grid Size",
    );
    draw_stepper_row(
        canvas,
        fonts,
        "Cols:",
        state.draft.cols,
        state.draft.cols > MIN_GRID_DIM,
        state.draft.cols < MAX_GRID_COLS,
        444.0,
        mouse_pos,
    );
    draw_stepper_row(
        canvas,
        fonts,
        "Rows:",
        state.draft.rows,
        state.draft.rows > MIN_GRID_DIM,
        state.draft.rows < MAX_GRID_ROWS,
        470.0,
        mouse_pos,
    );

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        127.0,
        501.0,
        11.0,
        (0xC0, 0xD4, 0xF0),
        TextAlign::Center,
        "R-Click:Spin  R:Rot  1-8",
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_tool_icon(
    canvas: &mut Canvas,
    bank: &TexBank,
    tool: EditorTool,
    cx: f32,
    cy: f32,
    size: f32,
    rot_deg: f32,
    pipe_col: GameColor,
    stationary_col: GameColor,
    portal_col: GameColor,
    mod_freeze: bool,
    alpha: f32,
) {
    let uv = [0.0, 0.0, 1.0, 1.0];
    match tool {
        EditorTool::Elbow => {
            if let Some((tid, _, _)) = bank.named("ElbowClearBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    rot_deg,
                    Some((pipe_col, false)),
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
            if mod_freeze && let Some((tid, _, _)) = bank.named("ElbowFrostBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    rot_deg,
                    None,
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
        }
        EditorTool::Straight => {
            if let Some((tid, _, _)) = bank.named("StraightClearBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    rot_deg,
                    Some((pipe_col, false)),
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
            if mod_freeze && let Some((tid, _, _)) = bank.named("StraightFrostBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    rot_deg,
                    None,
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
        }
        EditorTool::Cross => {
            if let Some((tid, _, _)) = bank.named("CrossClearBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    rot_deg,
                    Some((pipe_col, false)),
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
            if mod_freeze && let Some((tid, _, _)) = bank.named("CrossFrostBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    rot_deg,
                    None,
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
        }
        EditorTool::Blocker => {
            if let Some((tid, _, _)) = bank.named("CementBlockerBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    0.0,
                    None,
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
        }
        EditorTool::Source => {
            if let Some((tid, _, _)) = bank.named("SpigotClearBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    rot_deg,
                    Some((stationary_col, false)),
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
            if let Some((tid, _, _)) = bank.named("PipeSpigotBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    rot_deg,
                    None,
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
        }
        EditorTool::Drain => {
            if let Some((tid, _, _)) = bank.named("DrainBaseBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    rot_deg,
                    None,
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
            if let Some((tid, _, _)) = bank.named("DrainRingBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    rot_deg,
                    Some((stationary_col, false)),
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
            if let Some((tid, _, _)) = bank.named("DrainPipeClearBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    rot_deg,
                    Some((stationary_col, false)),
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
        }
        EditorTool::Portal => {
            if let Some((tid, _, _)) = bank.named("YellowPortalBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    rot_deg,
                    Some((portal_col, false)),
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
            if let Some((tid, _, _)) = bank.named("DrainPipeClearBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    size,
                    size,
                    rot_deg,
                    Some((stationary_col, false)),
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
        }
        EditorTool::Rotate => {
            if let Some((tid, _, _)) = bank.named("RotateCursorBitmap") {
                canvas.draw_tex_uv_rotated_blur(
                    tid,
                    uv,
                    cx,
                    cy,
                    30.0,
                    30.0,
                    0.0,
                    None,
                    1.0,
                    alpha,
                    false,
                    (0.0, 0.0),
                );
            }
        }
    }
}

fn render_bottom_bar(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    state: &EditorState,
    mouse_pos: (f32, f32),
) {
    canvas.fill_rect_alpha(
        Rect::new(0.0, 532.0, LOGICAL_W as f32, 68.0),
        0x05,
        0x0D,
        0x2B,
        175,
    );

    draw_editor_link(
        canvas,
        fonts,
        Rect::new(16.0, 538.0, 68.0, 22.0),
        16.0,
        539.0,
        15.0,
        TextAlign::Left,
        "undo",
        None,
        !state.undo_stack.is_empty(),
        mouse_pos,
    );
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(90.0, 538.0, 68.0, 22.0),
        90.0,
        539.0,
        15.0,
        TextAlign::Left,
        "redo",
        None,
        !state.redo_stack.is_empty(),
        mouse_pos,
    );
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(164.0, 538.0, 74.0, 22.0),
        164.0,
        539.0,
        15.0,
        TextAlign::Left,
        "clear",
        None,
        true,
        mouse_pos,
    );

    let can_solve = state.validation_issues.is_empty();
    let has_sol = matches!(state.solve_status, SolveStatus::Solvable(_));
    let path_lbl = if state.show_solution_path {
        "path: on"
    } else {
        "path: off"
    };
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(16.0, 566.0, 108.0, 22.0),
        16.0,
        567.0,
        15.0,
        TextAlign::Left,
        "auto-par",
        None,
        can_solve,
        mouse_pos,
    );
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(132.0, 566.0, 128.0, 22.0),
        132.0,
        567.0,
        15.0,
        TextAlign::Left,
        path_lbl,
        if state.show_solution_path {
            Some((0x8E, 0xFF, 0x8A))
        } else {
            None
        },
        has_sol,
        mouse_pos,
    );

    for (r, label) in [
        (Rect::new(284.0, 538.0, 94.0, 53.0), "props"),
        (Rect::new(386.0, 538.0, 94.0, 53.0), "verify"),
        (Rect::new(488.0, 538.0, 94.0, 53.0), "test"),
        (Rect::new(590.0, 538.0, 94.0, 53.0), "save"),
        (Rect::new(692.0, 538.0, 94.0, 53.0), "back"),
    ] {
        draw_modal_button(
            canvas,
            bank,
            fonts,
            r,
            label,
            r.contains(mouse_pos.0, mouse_pos.1),
            true,
        );
    }
}

pub fn build_level_props_buttons(props: &LevelPropsDraft) -> Vec<UiButton> {
    let mut out = vec![
        UiButton {
            id: "pr_title",
            rect: Rect::new(264.0, 92.0, 380.0, 26.0),
            action: UiAction::PropsEditTitle,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_desc",
            rect: Rect::new(264.0, 124.0, 380.0, 42.0),
            action: UiAction::PropsEditDesc,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_bg",
            rect: Rect::new(430.0, 178.0, 214.0, 22.0),
            action: UiAction::PropsCycleBg,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_liq",
            rect: Rect::new(430.0, 206.0, 214.0, 22.0),
            action: UiAction::PropsCycleLiquid,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_delay_dec",
            rect: Rect::new(516.0, 234.0, 28.0, 22.0),
            action: UiAction::PropsAdjustDelay(-1),
            enabled: props.start_delay > 0.0,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_delay_inc",
            rect: Rect::new(616.0, 234.0, 28.0, 22.0),
            action: UiAction::PropsAdjustDelay(1),
            enabled: props.start_delay < MAX_START_DELAY,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_speed_dec",
            rect: Rect::new(516.0, 262.0, 28.0, 22.0),
            action: UiAction::PropsAdjustSpeed(-1),
            enabled: props.liquid_speed > MIN_LIQUID_SPEED + 0.001,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_speed_inc",
            rect: Rect::new(616.0, 262.0, 28.0, 22.0),
            action: UiAction::PropsAdjustSpeed(1),
            enabled: props.liquid_speed < MAX_LIQUID_SPEED - 0.001,
            hover_sfx: Some("PopTwoSound"),
        },
    ];

    for (i, &(_, slot, btn_id)) in PIPE_COLOR_SLOTS.iter().enumerate() {
        let col = i % 2;
        let row = i / 2;
        let rx = 152.0 + col as f32 * 256.0;
        let ry = 320.0 + row as f32 * 26.0;
        out.push(UiButton {
            id: btn_id,
            rect: Rect::new(rx + 114.0, ry, 120.0, 22.0),
            action: UiAction::PropsCyclePipeColor(slot),
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        });
    }

    out.extend([
        UiButton {
            id: "pr_autopar",
            rect: Rect::new(520.0, 404.0, 124.0, 22.0),
            action: UiAction::PropsAutoPar,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_pm_dec",
            rect: Rect::new(224.0, 432.0, 24.0, 22.0),
            action: UiAction::PropsAdjustParMoves(-1),
            enabled: props.par_moves > 1,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_pm_inc",
            rect: Rect::new(286.0, 432.0, 24.0, 22.0),
            action: UiAction::PropsAdjustParMoves(1),
            enabled: props.par_moves < 99,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_pp_dec",
            rect: Rect::new(386.0, 432.0, 24.0, 22.0),
            action: UiAction::PropsAdjustParPipes(-1),
            enabled: props.par_pipes > 1,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_pp_inc",
            rect: Rect::new(448.0, 432.0, 24.0, 22.0),
            action: UiAction::PropsAdjustParPipes(1),
            enabled: props.par_pipes < 80,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_pt_dec",
            rect: Rect::new(548.0, 432.0, 24.0, 22.0),
            action: UiAction::PropsAdjustParTimeSec(-2),
            enabled: props.par_time_ms > 3000,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_pt_inc",
            rect: Rect::new(622.0, 432.0, 24.0, 22.0),
            action: UiAction::PropsAdjustParTimeSec(2),
            enabled: props.par_time_ms < 180_000,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_test_diff",
            rect: Rect::new(430.0, 462.0, 214.0, 22.0),
            action: UiAction::PropsCycleTestDiff,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "pr_done",
            rect: Rect::new(294.0, 492.0, 94.0, 53.0),
            action: UiAction::PropsApply,
            enabled: true,
            hover_sfx: None,
        },
        UiButton {
            id: "pr_cancel",
            rect: Rect::new(412.0, 492.0, 94.0, 53.0),
            action: UiAction::CloseTopModal,
            enabled: true,
            hover_sfx: None,
        },
    ]);

    out
}

const PIPE_COLOR_SLOTS: [(&str, PipeColorSlot, &str); 6] = [
    ("Mover:", PipeColorSlot::Mover, "pr_col_mover"),
    ("Gate:", PipeColorSlot::Gate, "pr_col_gate"),
    ("Rotater:", PipeColorSlot::Rotater, "pr_col_rotater"),
    ("Frozen:", PipeColorSlot::Freeze, "pr_col_freeze"),
    ("Fixed:", PipeColorSlot::Stationary, "pr_col_fixed"),
    ("Magic:", PipeColorSlot::Magic, "pr_col_magic"),
];

pub fn render_level_props_modal(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    props: &LevelPropsDraft,
    mouse_pos: (f32, f32),
) {
    draw_modal_bubble(canvas, Rect::new(124.0, 42.0, 552.0, 516.0));
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        400.0,
        56.0,
        24.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Center,
        "-Level Properties-",
    );

    // Title & Description recessed fields
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        152.0,
        96.0,
        15.0,
        (255, 255, 255),
        TextAlign::Left,
        "Title:",
    );
    let r_title_hit = Rect::new(264.0, 92.0, 380.0, 26.0);
    let title_hov = r_title_hit.contains(mouse_pos.0, mouse_pos.1);
    canvas.draw_rounded_rect(
        Rect::new(264.0, 92.0, 314.0, 26.0),
        6.0,
        (0x05, 0x0D, 0x2B, 204),
        Some(if title_hov {
            (0x59, 0x95, 0xC1, 255, 2.0)
        } else {
            (0x0E, 0x15, 0x32, 255, 2.0)
        }),
    );
    let title_trimmed = props.title.trim();
    if title_trimmed.is_empty() {
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            272.0,
            97.0,
            14.0,
            (0x88, 0xA0, 0xC0),
            TextAlign::Left,
            "(untitled)",
        );
    } else {
        let title_fit = truncate_to_width(fonts, FontFamily::Neuropol, title_trimmed, 14.0, 298.0);
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            272.0,
            97.0,
            14.0,
            (255, 255, 255),
            TextAlign::Left,
            &title_fit,
        );
    }
    draw_editor_link(
        canvas,
        fonts,
        r_title_hit,
        644.0,
        96.0,
        15.0,
        TextAlign::Right,
        "[edit]",
        Some((0x42, 0x83, 0xF3)),
        true,
        mouse_pos,
    );

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        152.0,
        130.0,
        15.0,
        (255, 255, 255),
        TextAlign::Left,
        "Description:",
    );
    let r_desc_hit = Rect::new(264.0, 124.0, 380.0, 42.0);
    let desc_hov = r_desc_hit.contains(mouse_pos.0, mouse_pos.1);
    canvas.draw_rounded_rect(
        Rect::new(264.0, 124.0, 314.0, 42.0),
        6.0,
        (0x05, 0x0D, 0x2B, 204),
        Some(if desc_hov {
            (0x59, 0x95, 0xC1, 255, 2.0)
        } else {
            (0x0E, 0x15, 0x32, 255, 2.0)
        }),
    );
    let desc_trimmed = props.description.trim();
    if desc_trimmed.is_empty() {
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            272.0,
            138.0,
            13.0,
            (0x88, 0xA0, 0xC0),
            TextAlign::Left,
            "(none)",
        );
    } else {
        let lines = fonts.wrap_lines(FontFamily::Neuropol, desc_trimmed, 13.0, 298.0);
        if lines.len() <= 1 {
            let line = truncate_to_width(fonts, FontFamily::Neuropol, &lines[0], 13.0, 298.0);
            fonts.draw(
                canvas,
                FontFamily::Neuropol,
                272.0,
                138.0,
                13.0,
                (255, 255, 255),
                TextAlign::Left,
                &line,
            );
        } else {
            let l1 = truncate_to_width(fonts, FontFamily::Neuropol, &lines[0], 13.0, 298.0);
            let l2_raw = if lines.len() > 2 {
                lines[1..].join(" ")
            } else {
                lines[1].clone()
            };
            let l2 = truncate_to_width(fonts, FontFamily::Neuropol, &l2_raw, 13.0, 298.0);
            fonts.draw(
                canvas,
                FontFamily::Neuropol,
                272.0,
                128.0,
                13.0,
                (255, 255, 255),
                TextAlign::Left,
                &l1,
            );
            fonts.draw(
                canvas,
                FontFamily::Neuropol,
                272.0,
                146.0,
                13.0,
                (255, 255, 255),
                TextAlign::Left,
                &l2,
            );
        }
    }
    draw_editor_link(
        canvas,
        fonts,
        r_desc_hit,
        644.0,
        136.0,
        15.0,
        TextAlign::Right,
        "[edit]",
        Some((0x42, 0x83, 0xF3)),
        true,
        mouse_pos,
    );

    // Environment & Flow
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        152.0,
        178.0,
        16.0,
        (255, 255, 255),
        TextAlign::Left,
        "Background:",
    );
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(430.0, 178.0, 214.0, 22.0),
        644.0,
        178.0,
        15.0,
        TextAlign::Right,
        &props.background_id,
        Some((0x42, 0x83, 0xF3)),
        true,
        mouse_pos,
    );

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        152.0,
        206.0,
        16.0,
        (255, 255, 255),
        TextAlign::Left,
        "Liquid Color:",
    );
    draw_color_swatch(
        canvas,
        Rect::new(506.0, 208.0, 20.0, 16.0),
        props.liquid_color,
        false,
    );
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(430.0, 206.0, 214.0, 22.0),
        644.0,
        206.0,
        15.0,
        TextAlign::Right,
        color_label(props.liquid_color),
        Some((0x42, 0x83, 0xF3)),
        true,
        mouse_pos,
    );

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        152.0,
        234.0,
        16.0,
        (255, 255, 255),
        TextAlign::Left,
        "Start Delay:",
    );
    draw_props_stepper(
        canvas,
        fonts,
        516.0,
        234.0,
        &format!("{:.0}s", props.start_delay),
        props.start_delay > 0.0,
        props.start_delay < MAX_START_DELAY,
        mouse_pos,
    );

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        152.0,
        262.0,
        16.0,
        (255, 255, 255),
        TextAlign::Left,
        "Flow Speed:",
    );
    let speed_str = if props.liquid_speed >= 10.0 {
        format!("{:.0}x", props.liquid_speed)
    } else if (props.liquid_speed * 2.0 - (props.liquid_speed * 2.0).round()).abs() < 0.01
        && props.liquid_speed >= 1.0
    {
        format!("{:.1}x", props.liquid_speed)
    } else {
        format!("{:.2}x", props.liquid_speed)
    };
    draw_props_stepper(
        canvas,
        fonts,
        516.0,
        262.0,
        &speed_str,
        props.liquid_speed > MIN_LIQUID_SPEED + 0.001,
        props.liquid_speed < MAX_LIQUID_SPEED - 0.001,
        mouse_pos,
    );

    // Pipe Theme Colors
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        152.0,
        294.0,
        16.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Left,
        "Pipe Theme Colors",
    );
    for (i, &(lbl, slot, _)) in PIPE_COLOR_SLOTS.iter().enumerate() {
        let col_idx = i % 2;
        let row_idx = i / 2;
        let rx = 152.0 + col_idx as f32 * 256.0;
        let ry = 320.0 + row_idx as f32 * 26.0;
        let c = match slot {
            PipeColorSlot::Mover => props.mover_color,
            PipeColorSlot::Rotater => props.rotater_color,
            PipeColorSlot::Stationary => props.stationary_color,
            PipeColorSlot::Gate => props.gate_color,
            PipeColorSlot::Freeze => props.freeze_color,
            PipeColorSlot::Magic => props.magic_color,
        };
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            rx,
            ry + 1.0,
            15.0,
            (255, 255, 255),
            TextAlign::Left,
            lbl,
        );
        draw_color_swatch(canvas, Rect::new(rx + 94.0, ry + 3.0, 16.0, 14.0), c, false);
        draw_editor_link(
            canvas,
            fonts,
            Rect::new(rx + 114.0, ry, 120.0, 22.0),
            rx + 234.0,
            ry,
            15.0,
            TextAlign::Right,
            color_label(c),
            Some((0x42, 0x83, 0xF3)),
            true,
            mouse_pos,
        );
    }

    // Par Targets + Playtest Difficulty
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        152.0,
        404.0,
        16.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Left,
        "Par Targets",
    );
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(520.0, 404.0, 124.0, 22.0),
        644.0,
        404.0,
        15.0,
        TextAlign::Right,
        "[auto-par]",
        Some((0x42, 0x83, 0xF3)),
        true,
        mouse_pos,
    );

    draw_par_cell(
        canvas,
        fonts,
        152.0,
        432.0,
        "Moves:",
        &props.par_moves.to_string(),
        224.0,
        286.0,
        props.par_moves > 1,
        props.par_moves < 99,
        mouse_pos,
    );
    draw_par_cell(
        canvas,
        fonts,
        324.0,
        432.0,
        "Pipes:",
        &props.par_pipes.to_string(),
        386.0,
        448.0,
        props.par_pipes > 1,
        props.par_pipes < 80,
        mouse_pos,
    );
    let pt_sec = props.par_time_ms / 1000;
    draw_par_cell(
        canvas,
        fonts,
        486.0,
        432.0,
        "Time:",
        &format!("{}s", pt_sec),
        548.0,
        622.0,
        props.par_time_ms > 3000,
        props.par_time_ms < 180_000,
        mouse_pos,
    );

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        152.0,
        462.0,
        15.0,
        (255, 255, 255),
        TextAlign::Left,
        "Playtest Difficulty:",
    );
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(430.0, 462.0, 214.0, 22.0),
        644.0,
        462.0,
        15.0,
        TextAlign::Right,
        props.test_difficulty.human_label(),
        Some((0x42, 0x83, 0xF3)),
        true,
        mouse_pos,
    );

    let r_done = Rect::new(294.0, 492.0, 94.0, 53.0);
    let r_cancel = Rect::new(412.0, 492.0, 94.0, 53.0);
    draw_modal_button(
        canvas,
        bank,
        fonts,
        r_done,
        "done",
        r_done.contains(mouse_pos.0, mouse_pos.1),
        true,
    );
    draw_modal_button(
        canvas,
        bank,
        fonts,
        r_cancel,
        "cancel",
        r_cancel.contains(mouse_pos.0, mouse_pos.1),
        true,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_par_cell(
    canvas: &mut Canvas,
    fonts: &FontSet,
    lx: f32,
    y: f32,
    label: &str,
    val: &str,
    dec_x: f32,
    inc_x: f32,
    can_dec: bool,
    can_inc: bool,
    mouse_pos: (f32, f32),
) {
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        lx,
        y + 2.0,
        14.0,
        (0xC0, 0xD4, 0xF0),
        TextAlign::Left,
        label,
    );
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(dec_x, y, 24.0, 22.0),
        dec_x + 12.0,
        y + 1.0,
        15.0,
        TextAlign::Center,
        "[-]",
        None,
        can_dec,
        mouse_pos,
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        (dec_x + 24.0 + inc_x) * 0.5,
        y + 2.0,
        14.0,
        (255, 255, 255),
        TextAlign::Center,
        val,
    );
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(inc_x, y, 24.0, 22.0),
        inc_x + 12.0,
        y + 1.0,
        15.0,
        TextAlign::Center,
        "[+]",
        None,
        can_inc,
        mouse_pos,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_props_stepper(
    canvas: &mut Canvas,
    fonts: &FontSet,
    dec_x: f32,
    y: f32,
    val: &str,
    can_dec: bool,
    can_inc: bool,
    mouse_pos: (f32, f32),
) {
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(dec_x, y, 28.0, 22.0),
        dec_x + 14.0,
        y,
        15.0,
        TextAlign::Center,
        "[-]",
        None,
        can_dec,
        mouse_pos,
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        dec_x + 64.0,
        y + 1.0,
        15.0,
        (255, 255, 255),
        TextAlign::Center,
        val,
    );
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(dec_x + 100.0, y, 28.0, 22.0),
        dec_x + 114.0,
        y,
        15.0,
        TextAlign::Center,
        "[+]",
        None,
        can_inc,
        mouse_pos,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_stepper_row(
    canvas: &mut Canvas,
    fonts: &FontSet,
    label: &str,
    val: usize,
    can_dec: bool,
    can_inc: bool,
    y: f32,
    mouse_pos: (f32, f32),
) {
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        30.0,
        y + 2.0,
        14.0,
        (255, 255, 255),
        TextAlign::Left,
        label,
    );
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(112.0, y, 28.0, 22.0),
        126.0,
        y + 1.0,
        15.0,
        TextAlign::Center,
        "[-]",
        None,
        can_dec,
        mouse_pos,
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        161.0,
        y + 2.0,
        15.0,
        (255, 255, 255),
        TextAlign::Center,
        &val.to_string(),
    );
    draw_editor_link(
        canvas,
        fonts,
        Rect::new(182.0, y, 28.0, 22.0),
        196.0,
        y + 1.0,
        15.0,
        TextAlign::Center,
        "[+]",
        None,
        can_inc,
        mouse_pos,
    );
}

fn draw_small_radio(
    canvas: &mut Canvas,
    fonts: &FontSet,
    x: f32,
    y: f32,
    label: &str,
    active: bool,
    enabled: bool,
) {
    let a = if enabled { 255 } else { 96 };
    if enabled {
        draw_radio(canvas, fonts, x, y - 2.0, "", active);
    } else {
        let cx = x + 8.0;
        let cy = y + 10.0;
        canvas.stroke_circle_alpha(cx, cy, 8.0, 1.5, (255, 255, 255, a));
        if active {
            canvas.fill_circle_alpha(cx, cy, 4.0, (255, 255, 255, a));
        }
    }
    let col = if enabled {
        (255, 255, 255)
    } else {
        (140, 150, 168)
    };
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        x + 24.0,
        y + 1.0,
        14.0,
        col,
        TextAlign::Left,
        label,
    );
}

fn draw_small_checkbox(
    canvas: &mut Canvas,
    fonts: &FontSet,
    x: f32,
    y: f32,
    label: &str,
    checked: bool,
    enabled: bool,
) {
    if enabled {
        draw_checkbox(canvas, fonts, x + 1.0, y, "", checked, false);
    } else {
        canvas.draw_rounded_rect(
            Rect::new(x + 1.0, y, 15.0, 15.0),
            0.0,
            (0, 0, 0, 60),
            Some((160, 170, 185, 110, 2.0)),
        );
    }
    let col = if enabled {
        (255, 255, 255)
    } else {
        (140, 150, 168)
    };
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        x + 24.0,
        y - 1.0,
        14.0,
        col,
        TextAlign::Left,
        label,
    );
}

fn draw_color_swatch(canvas: &mut Canvas, rect: Rect, color: GameColor, selected: bool) {
    let (r, g, b) = color.rgb();
    let border = if selected {
        (0xC9, 0xDA, 0x41, 255, 2.5)
    } else {
        (255, 255, 255, 190, 1.5)
    };
    canvas.draw_rounded_rect(rect, 3.0, (r, g, b, 255), Some(border));
}

#[allow(clippy::too_many_arguments)]
fn draw_editor_link(
    canvas: &mut Canvas,
    fonts: &FontSet,
    hit: Rect,
    x: f32,
    y: f32,
    size: f32,
    align: TextAlign,
    label: &str,
    idle_col: Option<(u8, u8, u8)>,
    enabled: bool,
    mouse_pos: (f32, f32),
) {
    let hovered = enabled && hit.contains(mouse_pos.0, mouse_pos.1);
    let alpha = if enabled { 1.0 } else { 0.38 };
    draw_text_button(
        canvas, fonts, x, y, size, align, label, hovered, idle_col, alpha,
    );
}

pub fn color_label(c: GameColor) -> &'static str {
    match c {
        GameColor::Red => "Red",
        GameColor::Orange => "Orange",
        GameColor::Yellow => "Yellow",
        GameColor::Green => "Green",
        GameColor::Blue => "Blue",
        GameColor::Violet => "Violet",
        GameColor::White => "White",
        GameColor::Grey => "Grey",
    }
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let head: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", head)
    }
}

fn truncate_to_width(
    fonts: &FontSet,
    fam: FontFamily,
    text: &str,
    size: f32,
    max_w: f32,
) -> String {
    if fonts.measure_width(fam, text, size) <= max_w {
        return text.to_string();
    }
    let ellipsis = "…";
    let ellipsis_w = fonts.measure_width(fam, ellipsis, size);
    let budget = (max_w - ellipsis_w).max(0.0);
    let mut head = String::new();
    for ch in text.chars() {
        head.push(ch);
        if fonts.measure_width(fam, &head, size) > budget {
            head.pop();
            break;
        }
    }
    let trimmed = head.trim_end();
    if trimmed.is_empty() && !head.is_empty() {
        format!("{}{}", head, ellipsis)
    } else {
        format!("{}{}", trimmed, ellipsis)
    }
}
