// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::generator::{GenDifficulty, MIN_GEN_COLS, MIN_GEN_ROWS};
use crate::game::level_io::{MAX_GRID_COLS, MAX_GRID_ROWS};
use crate::game::{Difficulty, MetricResult, Trophy};
use crate::render::hdr::{HDR_PAPER_WHITE_DEFAULT, HDR_PEAK_DEFAULT};
use directories::ProjectDirs;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndlessRules {
    MatchLevel,
    Fixed(Difficulty),
}

impl EndlessRules {
    pub fn label(self) -> &'static str {
        match self {
            Self::MatchLevel => "Match Level",
            Self::Fixed(Difficulty::Easy) => "Relaxed (Easy)",
            Self::Fixed(Difficulty::Medium) => "Casual (Medium)",
            Self::Fixed(Difficulty::Hard) => "Hardcore (Hard)",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::MatchLevel => Self::Fixed(Difficulty::Easy),
            Self::Fixed(Difficulty::Easy) => Self::Fixed(Difficulty::Medium),
            Self::Fixed(Difficulty::Medium) => Self::Fixed(Difficulty::Hard),
            Self::Fixed(Difficulty::Hard) => Self::MatchLevel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndlessElement {
    Movable,
    Rotatable,
    Immovable,
    Frosted,
    Rocks,
    GateMagic,
    Portals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EndlessElements {
    pub movable: bool,
    pub rotatable: bool,
    pub immovable: bool,
    pub frosted: bool,
    pub rocks: bool,
    pub gate_magic: bool,
    pub portals: bool,
}

impl Default for EndlessElements {
    fn default() -> Self {
        Self {
            movable: true,
            rotatable: true,
            immovable: true,
            frosted: true,
            rocks: true,
            gate_magic: true,
            portals: true,
        }
    }
}

impl EndlessElements {
    pub fn has_interactive_pipes(self) -> bool {
        self.movable || self.rotatable
    }

    pub fn toggle(&mut self, elem: EndlessElement) {
        match elem {
            EndlessElement::Movable => self.movable = !self.movable,
            EndlessElement::Rotatable => self.rotatable = !self.rotatable,
            EndlessElement::Immovable => self.immovable = !self.immovable,
            EndlessElement::Frosted => self.frosted = !self.frosted,
            EndlessElement::Rocks => self.rocks = !self.rocks,
            EndlessElement::GateMagic => self.gate_magic = !self.gate_magic,
            EndlessElement::Portals => self.portals = !self.portals,
        }
    }

    pub fn sanitized(mut self) -> Self {
        if !self.has_interactive_pipes() {
            self.movable = true;
            self.rotatable = true;
        }
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EndlessConfig {
    pub easy: bool,
    pub medium: bool,
    pub hard: bool,
    pub custom: bool,
    pub elements: EndlessElements,
    pub max_cols: usize,
    pub max_rows: usize,
    pub rules: EndlessRules,
}

impl Default for EndlessConfig {
    fn default() -> Self {
        Self {
            easy: true,
            medium: true,
            hard: true,
            custom: false,
            elements: EndlessElements::default(),
            max_cols: 8,
            max_rows: 6,
            rules: EndlessRules::MatchLevel,
        }
    }
}

impl EndlessConfig {
    pub fn sanitized(mut self) -> Self {
        self.max_cols = self.max_cols.clamp(MIN_GEN_COLS, MAX_GRID_COLS);
        self.max_rows = self.max_rows.clamp(MIN_GEN_ROWS, MAX_GRID_ROWS);
        self.elements = self.elements.sanitized();
        if !self.custom && !(self.easy || self.medium || self.hard) {
            self.easy = true;
            self.medium = true;
            self.hard = true;
        }
        self
    }

    pub fn any_enabled(self) -> bool {
        if self.custom {
            self.elements.has_interactive_pipes()
        } else {
            self.easy || self.medium || self.hard
        }
    }

    pub fn toggle(&mut self, diff: GenDifficulty) {
        if self.custom {
            return;
        }
        match diff {
            GenDifficulty::Easy => self.easy = !self.easy,
            GenDifficulty::Medium => self.medium = !self.medium,
            GenDifficulty::Hard => self.hard = !self.hard,
        }
    }

    pub fn toggle_custom(&mut self) {
        self.custom = !self.custom;
        if !self.custom && !(self.easy || self.medium || self.hard) {
            self.easy = true;
            self.medium = true;
            self.hard = true;
        }
    }

    pub fn toggle_element(&mut self, elem: EndlessElement) {
        if self.custom {
            self.elements.toggle(elem);
        }
    }

    pub fn adjust_max_cols(&mut self, delta: i8) {
        self.max_cols = (self.max_cols as isize + delta as isize)
            .clamp(MIN_GEN_COLS as isize, MAX_GRID_COLS as isize) as usize;
    }

    pub fn adjust_max_rows(&mut self, delta: i8) {
        self.max_rows = (self.max_rows as isize + delta as isize)
            .clamp(MIN_GEN_ROWS as isize, MAX_GRID_ROWS as isize) as usize;
    }

    pub fn enabled_difficulties(self) -> Vec<GenDifficulty> {
        let mut out = Vec::with_capacity(3);
        if self.easy {
            out.push(GenDifficulty::Easy);
        }
        if self.medium {
            out.push(GenDifficulty::Medium);
        }
        if self.hard {
            out.push(GenDifficulty::Hard);
        }
        out
    }

    pub fn pick_gen_difficulty(self, rng: &mut impl RngExt) -> GenDifficulty {
        let pool = self.enabled_difficulties();
        if pool.is_empty() {
            GenDifficulty::Medium
        } else {
            pool[rng.random_range(0..pool.len())]
        }
    }

    pub fn play_difficulty_for(self, gen_diff: GenDifficulty) -> Difficulty {
        match self.rules {
            EndlessRules::Fixed(d) => d,
            EndlessRules::MatchLevel => match gen_diff {
                GenDifficulty::Easy => Difficulty::Easy,
                GenDifficulty::Medium => Difficulty::Medium,
                GenDifficulty::Hard => Difficulty::Hard,
            },
        }
    }

    pub fn play_difficulty_for_custom(self) -> Difficulty {
        match self.rules {
            EndlessRules::Fixed(d) => d,
            EndlessRules::MatchLevel => Difficulty::Medium,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LevelRecord {
    pub best_trophy: Trophy,
    pub best_score: u32,
    pub best_moves: Option<u32>,
    pub best_time_ms: Option<u32>,
}

impl LevelRecord {
    pub fn is_better_or_update(&mut self, res: MetricResult, moves: u32, time_ms: u32) {
        let better_trophy = res.trophy > self.best_trophy;
        let same_trophy_higher =
            res.trophy == self.best_trophy && res.total_score >= self.best_score;
        if better_trophy || same_trophy_higher {
            self.best_trophy = res.trophy;
            self.best_score = res.total_score;
            self.best_moves = Some(moves);
            self.best_time_ms = Some(time_ms);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub sfx_volume: f32,
    pub music_volume: f32,
    pub color_blind: bool,
    pub fullscreen: bool,
    pub hdr: bool,
    pub hdr_paper_white_nits: f32,
    pub hdr_peak_nits: f32,
    pub target_fps: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sfx_volume: 1.0,
            music_volume: 0.46,
            color_blind: false,
            fullscreen: false,
            hdr: false,
            hdr_paper_white_nits: HDR_PAPER_WHITE_DEFAULT,
            hdr_peak_nits: HDR_PEAK_DEFAULT,
            target_fps: 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileData {
    pub name: String,
    pub level_index: usize,
    pub savable_level_index: usize,
    pub difficulty: Difficulty,
    pub savable_difficulty: Difficulty,
    pub match_level_index: i32,
    pub mirror_level_index: i32,
    pub awards: [bool; 4],
    pub records: HashMap<usize, LevelRecord>,
    pub endless_config: EndlessConfig,
}

impl Default for ProfileData {
    fn default() -> Self {
        Self {
            name: "Player 1".to_string(),
            level_index: 0,
            savable_level_index: 0,
            difficulty: Difficulty::Medium,
            savable_difficulty: Difficulty::Medium,
            match_level_index: -1,
            mirror_level_index: -1,
            awards: [false; 4],
            records: HashMap::new(),
            endless_config: EndlessConfig::default(),
        }
    }
}

impl ProfileData {
    pub fn has_saved_game(&self) -> bool {
        !self.records.is_empty()
    }

    pub fn total_score(&self) -> u32 {
        let lvl_sum: u32 = self.records.values().map(|r| r.best_score).sum();
        let match_pts = (self.match_level_index.max(0) as u32) * 200;
        let mirror_pts = (self.mirror_level_index.max(0) as u32) * 350;
        lvl_sum + match_pts + mirror_pts
    }

    pub fn is_match_unlocked(&self) -> bool {
        self.awards[0]
    }

    pub fn is_mirror_unlocked(&self) -> bool {
        self.awards[1]
    }

    pub fn all_platinum(&self, total_levels: usize) -> bool {
        total_levels > 0
            && (0..total_levels).all(|idx| {
                self.records
                    .get(&idx)
                    .is_some_and(|r| r.best_trophy == Trophy::Platinum)
            })
    }

    pub fn is_level_unlocked(&self, idx: usize) -> bool {
        self.has_saved_game() && idx <= self.records.len()
    }

    pub fn record_win(&mut self, level_idx: usize, res: MetricResult, moves: u32, time_ms: u32) {
        self.records
            .entry(level_idx)
            .or_default()
            .is_better_or_update(res, moves, time_ms);
    }

    pub fn reset_campaign(&mut self) {
        self.level_index = 0;
        self.savable_level_index = 0;
        self.match_level_index = -1;
        self.mirror_level_index = -1;
        self.awards = [false; 4];
        self.records.clear();
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SaveData {
    pub current_profile: Option<String>,
    pub profiles: Vec<ProfileData>,
    pub config: AppConfig,
    #[cfg(test)]
    #[serde(skip)]
    pub save_count: std::cell::Cell<u32>,
}

impl SaveData {
    pub fn save_path() -> PathBuf {
        if let Some(proj) = ProjectDirs::from("com", "aqualux-rs", "aqualux-rs") {
            let dir = proj.data_local_dir();
            let dir_ok = match fs::symlink_metadata(dir) {
                Ok(meta) => !meta.file_type().is_symlink() && meta.file_type().is_dir(),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir_all(dir).is_ok()
                }
                Err(_) => false,
            };
            if dir_ok {
                return dir.join("save.json");
            }
        }
        PathBuf::from("save.json")
    }

    pub(crate) fn from_json_str(raw: &str) -> Option<Self> {
        let mut parsed = serde_json::from_str::<SaveData>(raw).ok()?;
        for prof in &mut parsed.profiles {
            prof.endless_config = prof.endless_config.sanitized();
        }
        Some(parsed)
    }

    pub fn load() -> Self {
        if cfg!(test) {
            return Self::default();
        }
        let path = Self::save_path();
        if let Ok(raw) = fs::read_to_string(&path)
            && let Some(parsed) = Self::from_json_str(&raw)
        {
            return parsed;
        }
        Self::default()
    }

    pub(crate) fn write_save_to_path(&self, path: &Path) -> Result<(), String> {
        if path.as_os_str().is_empty()
            || path.file_name().is_none()
            || path.components().any(|c| matches!(c, Component::ParentDir))
            || !matches!(path.components().next_back(), Some(Component::Normal(_)))
        {
            return Err(format!("Refusing unsafe save path '{}'", path.display()));
        }
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            if let Ok(meta) = fs::symlink_metadata(parent)
                && (meta.file_type().is_symlink() || !meta.file_type().is_dir())
            {
                return Err(format!(
                    "Refusing symlink or non-directory save parent '{}'",
                    parent.display()
                ));
            }
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create '{}': {}", parent.display(), e))?;
            let meta = fs::symlink_metadata(parent)
                .map_err(|e| format!("Cannot inspect '{}': {}", parent.display(), e))?;
            if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
                return Err(format!(
                    "Refusing symlink or non-directory save parent '{}'",
                    parent.display()
                ));
            }
        }
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.file_type().is_file() => {
                return Err(format!(
                    "Refusing to overwrite symlink or non-file '{}'",
                    path.display()
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Cannot inspect '{}': {}", path.display(), e)),
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize save data: {}", e))?;
        fs::write(path, json).map_err(|e| format!("Failed to write '{}': {}", path.display(), e))
    }

    pub fn save(&self) {
        #[cfg(test)]
        {
            self.save_count.set(self.save_count.get() + 1);
        }
        #[cfg(not(test))]
        let _ = self.write_save_to_path(&Self::save_path());
    }

    pub fn has_profile(&self) -> bool {
        self.active_profile().is_some()
    }

    pub fn active_profile(&self) -> Option<&ProfileData> {
        let cur = self.current_profile.as_deref()?;
        self.profiles.iter().find(|p| p.name == cur)
    }

    pub fn active_profile_mut(&mut self) -> Option<&mut ProfileData> {
        let cur = self.current_profile.as_deref()?;
        self.profiles.iter_mut().find(|p| p.name == cur)
    }

    pub fn set_or_create_profile(&mut self, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Some(existing) = self.profiles.iter().find(|p| p.name == trimmed) {
            self.current_profile = Some(existing.name.clone());
        } else {
            let prof = ProfileData {
                name: trimmed.to_string(),
                ..Default::default()
            };
            self.current_profile = Some(prof.name.clone());
            self.profiles.push(prof);
        }
        self.save();
    }

    pub fn apply_profile_list(&mut self, selected: &str, remaining_names: &[String]) {
        self.profiles
            .retain(|p| remaining_names.iter().any(|n| n == &p.name));
        for n in remaining_names {
            if !self.profiles.iter().any(|p| p.name == *n) {
                self.profiles.push(ProfileData {
                    name: n.clone(),
                    ..Default::default()
                });
            }
        }
        self.current_profile = Some(selected.to_string());
        self.save();
    }
}
