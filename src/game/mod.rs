// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

pub mod awards;
pub mod generator;
pub mod level_io;
pub mod level_sim;
pub mod level_step;
pub mod match_sim;
pub mod path;
pub mod scoring;
pub mod solver;
pub mod solver_route;
pub mod tile;
pub mod types;

pub use awards::{AwardKind, Difficulty, Trophy};
pub use level_io::parse_levels_xml;
pub use level_sim::{LevelSim, SimEvent};
pub use match_sim::MatchSim;
pub use scoring::MetricResult;
pub use tile::Tile;
pub use types::{FillPass, FlowTimerState, GameColor, LevelDef, PieceShape, PieceType, Side};
