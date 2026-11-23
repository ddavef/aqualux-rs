// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::{Difficulty, MetricResult, Trophy};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

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
    pub target_fps: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sfx_volume: 1.0,
            music_volume: 0.46,
            color_blind: false,
            fullscreen: false,
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

    pub fn load() -> Self {
        let path = Self::save_path();
        if let Ok(raw) = fs::read_to_string(&path)
            && let Ok(parsed) = serde_json::from_str::<SaveData>(&raw)
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
        if cfg!(test) {
            return;
        }
        let _ = self.write_save_to_path(&Self::save_path());
    }

    pub fn has_profile(&self) -> bool {
        self.active_profile().is_some()
    }

    pub fn active_profile(&self) -> Option<&ProfileData> {
        let cur = self.current_profile.as_deref()?;
        self.profiles
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(cur))
    }

    pub fn active_profile_mut(&mut self) -> Option<&mut ProfileData> {
        let cur = self.current_profile.as_deref()?;
        self.profiles
            .iter_mut()
            .find(|p| p.name.eq_ignore_ascii_case(cur))
    }

    pub fn set_or_create_profile(&mut self, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Some(existing) = self
            .profiles
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(trimmed))
        {
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
        self.profiles.retain(|p| {
            remaining_names
                .iter()
                .any(|n| n.eq_ignore_ascii_case(&p.name))
        });
        for n in remaining_names {
            if !self.profiles.iter().any(|p| p.name.eq_ignore_ascii_case(n)) {
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
