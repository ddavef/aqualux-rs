// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Side {
    Up = 0,
    Right = 1,
    Down = 2,
    Left = 3,
    Center = 4,
}

impl Side {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Up),
            1 => Some(Self::Right),
            2 => Some(Self::Down),
            3 => Some(Self::Left),
            4 => Some(Self::Center),
            _ => None,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Right => Self::Left,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Center => Self::Center,
        }
    }

    pub fn delta(self) -> (i32, i32) {
        match self {
            Self::Up => (0, -1),
            Self::Right => (1, 0),
            Self::Down => (0, 1),
            Self::Left => (-1, 0),
            Self::Center => (0, 0),
        }
    }

    pub fn rotate_cw(self, steps: u8) -> Self {
        match self {
            Self::Center => Self::Center,
            _ => Self::from_u8((self as u8 + steps) % 4).unwrap_or(Self::Up),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PieceShape {
    Elbow,
    Straight,
    Cross,
    Spigot,
    Drain,
    Block,
}

impl PieceShape {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "elbow" => Some(Self::Elbow),
            "straight" => Some(Self::Straight),
            "cross" => Some(Self::Cross),
            "spigot" => Some(Self::Spigot),
            "drain" => Some(Self::Drain),
            "block" => Some(Self::Block),
            _ => None,
        }
    }

    pub fn default_ports(self) -> [bool; 4] {
        match self {
            Self::Elbow => [true, true, false, false],
            Self::Straight => [true, false, true, false],
            Self::Cross => [true, true, true, true],
            Self::Spigot => [false, false, true, false],
            Self::Drain => [false, false, true, false],
            Self::Block => [false, false, false, false],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PieceType {
    Source,
    Pipe,
    Drain,
    Blocker,
    Portal,
    FrozenPipe,
}

impl PieceType {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "source" => Some(Self::Source),
            "pipe" => Some(Self::Pipe),
            "drain" => Some(Self::Drain),
            "blocker" => Some(Self::Blocker),
            "portal" => Some(Self::Portal),
            "frozenpipe" | "frozen_pipe" => Some(Self::FrozenPipe),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameColor {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Violet,
    White,
    Grey,
}

impl GameColor {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_ascii_uppercase().as_str() {
            "RED" => Some(Self::Red),
            "ORANGE" => Some(Self::Orange),
            "YELLOW" => Some(Self::Yellow),
            "GREEN" => Some(Self::Green),
            "BLUE" => Some(Self::Blue),
            "VIOLET" => Some(Self::Violet),
            "WHITE" => Some(Self::White),
            "GREY" | "GRAY" => Some(Self::Grey),
            _ => None,
        }
    }

    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Red => (0x8A, 0x3E, 0x35),
            Self::Orange => (0xE6, 0x73, 0x00),
            Self::Yellow => (0xD2, 0xD2, 0x00),
            Self::Green => (0x3A, 0xF4, 0x04),
            Self::Blue => (0x66, 0xB3, 0xFF),
            Self::Violet => (0xA9, 0x5C, 0xAF),
            Self::White => (0xE6, 0xE6, 0xE6),
            Self::Grey => (0x3F, 0x3F, 0x3F),
        }
    }

    /// ColorMatrix parameters: `(hue_deg, brightness, saturation)` for pipe and liquid tinting.
    pub fn matrix_props(self) -> (f32, f32, f32) {
        match self {
            Self::Red => (289.0, 0.0, 1.5),
            Self::Orange => (318.0, 1.0, 1.5),
            Self::Yellow => (0.0, 1.0, 1.0),
            Self::Green => (23.0, 1.0, 1.1),
            Self::Blue => (131.0, 1.0, 0.98),
            Self::Violet => (210.0, 1.0, 1.4),
            Self::White => (278.0, 1.0, 0.0),
            Self::Grey => (278.0, 0.0, 0.0),
        }
    }

    pub fn color_blind_char(self) -> &'static str {
        match self {
            Self::Red => "R",
            Self::Orange => "O",
            Self::Yellow => "Y",
            Self::Green => "G",
            Self::Blue => "B",
            Self::Violet => "V",
            Self::White => "W",
            Self::Grey => "X",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TileDef {
    pub x: usize,
    pub y: usize,
    pub shape: PieceShape,
    pub piece_type: PieceType,
    pub direction: u8,
    pub is_moveable: bool,
    pub can_rotate: bool,
    pub is_gate: bool,
    pub is_freeze: bool,
    pub is_magic: bool,
    pub secondary_color: Option<GameColor>,
}

impl TileDef {
    pub fn dummy_blocker() -> Self {
        Self {
            x: 0,
            y: 0,
            shape: PieceShape::Block,
            piece_type: PieceType::Blocker,
            direction: 0,
            is_moveable: false,
            can_rotate: false,
            is_gate: false,
            is_freeze: false,
            is_magic: false,
            secondary_color: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TutorialMsg {
    pub tiles: Vec<(usize, usize)>,
    pub wait_for_select: Option<(usize, usize)>,
    pub wait_for_swap: Vec<(usize, usize)>,
    pub wait_for_drag: Vec<(usize, usize)>,
    pub hold_hilight: bool,
    pub text: String,
}

impl TutorialMsg {
    pub fn click_to_continue(&self) -> bool {
        self.wait_for_swap.is_empty()
            && self.wait_for_drag.is_empty()
            && self.wait_for_select.is_none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LevelDef {
    pub index: usize,
    pub title: String,
    pub description: String,
    pub background_id: String,
    pub liquid_color: GameColor,
    pub start_delay: f32,
    pub liquid_speed: f32,
    pub rotater_color: GameColor,
    pub mover_color: GameColor,
    pub stationary_color: GameColor,
    pub gate_color: GameColor,
    pub freeze_color: GameColor,
    pub magic_color: GameColor,
    pub par_moves: u32,
    pub par_pipes: u32,
    pub par_time_ms: u32,
    pub cols: usize,
    pub rows: usize,
    pub tiles: Vec<TileDef>,
    pub tutorials: Vec<TutorialMsg>,
}

/// One active or finished liquid traversal through a tile.
#[derive(Debug, Clone, Copy)]
pub struct FillPass {
    pub entry: Side,
    pub exit: Side,
    pub progress: f32,
    pub elapsed: f32,
    pub completed: bool,
}

/// Active timer state attached to a Source or Portal tile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowTimerState {
    pub total: f32,
    pub remaining: f32,
    pub age: f32,
    pub last_alarm_sec: i32,
    pub warning_red: bool,
    pub warn_elapsed: Option<f32>,
    pub fading: bool,
    pub fade_elapsed: f32,
}

impl FlowTimerState {
    pub fn new(seconds: f32) -> Self {
        Self {
            total: seconds,
            remaining: seconds,
            age: 0.0,
            last_alarm_sec: seconds.ceil() as i32,
            warning_red: false,
            warn_elapsed: None,
            fading: false,
            fade_elapsed: 0.0,
        }
    }

    pub fn trigger_fade(&mut self) {
        if !self.fading {
            self.fading = true;
            self.fade_elapsed = 0.0;
        }
    }
}
