// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::custom::sanitize_pack_id;
use crate::game::Difficulty;
use crate::game::generator::{GenDifficulty, generate_custom_level};
use crate::game::level_io::{CustomPackFile, CustomPackMeta, default_starter_level};
use crate::ui::widgets::{ModalState, QuestionId, TextPromptPurpose, UiAction};
use crate::ui::{GameApp, PlayMode, Screen};
use std::path::Path;

const PACKS_PER_PAGE: usize = 8;
const LEVELS_PER_PAGE: usize = 7;

impl GameApp {
    pub fn open_custom_hub(&mut self, play_focused: bool) {
        self.play_mode = PlayMode::Campaign;
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

        self.custom_packs.reload();
        if self.custom_packs.packs.is_empty() && !play_focused {
            let starter = CustomPackFile {
                meta: CustomPackMeta {
                    name: "My Levels".to_string(),
                    author: self
                        .save
                        .active_profile()
                        .map(|p| p.name.clone())
                        .unwrap_or_default(),
                    description: "Custom level pack".to_string(),
                },
                levels: vec![default_starter_level(0)],
            };
            let _ = self.custom_packs.save_pack("my-levels", starter);
        }

        self.custom_hub.play_focused = play_focused;
        self.custom_hub.clamp_selection(&self.custom_packs);
        self.bg_anim.load("NightHills");
        self.bg_anim.lighten(1.9);
        self.audio.clear_delayed();
        self.audio.stop_music();
        self.audio.play_music("LittleRayMusic", true);
        self.screen = Screen::CustomHub;
    }

    pub fn handle_custom_hub_action(&mut self, action: UiAction) {
        match action {
            UiAction::HubBackToTitle => {
                self.play_mode = PlayMode::Campaign;
                self.return_to_main_menu();
            }
            UiAction::HubOpenFolder => match self.custom_packs.open_packs_folder() {
                Ok(()) => self.show_status_toast(&format!(
                    "Opened {}",
                    self.custom_packs.primary_dir.display()
                )),
                Err(e) => self.show_status_toast(&e),
            },
            UiAction::HubReloadPacks => {
                self.custom_packs.reload();
                self.custom_hub.clamp_selection(&self.custom_packs);
                self.show_status_toast(&format!(
                    "Loaded {} custom pack(s)",
                    self.custom_packs.packs.len()
                ));
            }
            UiAction::HubSelectPack(idx) => {
                self.custom_hub.selected_pack = idx;
                self.custom_hub.selected_level = 0;
                self.custom_hub.level_page = 0;
                self.custom_hub.clamp_selection(&self.custom_packs);
                self.custom_hub.sync_pages_to_selection();
            }
            UiAction::HubPackPrevPage => {
                self.custom_hub.pack_page = self.custom_hub.pack_page.saturating_sub(1);
            }
            UiAction::HubPackNextPage => {
                let total = self
                    .custom_packs
                    .packs
                    .len()
                    .max(1)
                    .div_ceil(PACKS_PER_PAGE);
                if self.custom_hub.pack_page + 1 < total {
                    self.custom_hub.pack_page += 1;
                }
            }
            UiAction::HubSelectLevel(idx) => {
                self.custom_hub.selected_level = idx;
                self.custom_hub.clamp_selection(&self.custom_packs);
                self.custom_hub.sync_pages_to_selection();
            }
            UiAction::HubLevelPrevPage => {
                self.custom_hub.level_page = self.custom_hub.level_page.saturating_sub(1);
            }
            UiAction::HubLevelNextPage => {
                let len = self
                    .custom_packs
                    .packs
                    .get(self.custom_hub.selected_pack)
                    .map(|p| p.pack.levels.len())
                    .unwrap_or(0);
                let total = len.max(1).div_ceil(LEVELS_PER_PAGE);
                if self.custom_hub.level_page + 1 < total {
                    self.custom_hub.level_page += 1;
                }
            }
            UiAction::HubNewPack => {
                self.modals.push(ModalState::TextPrompt {
                    purpose: TextPromptPurpose::NewCustomPack,
                    prompt: "Enter new pack name:".to_string(),
                    input: String::new(),
                    max_len: 32,
                });
            }
            UiAction::HubRenamePack => {
                if let Some(p) = self.custom_packs.packs.get(self.custom_hub.selected_pack)
                    && p.writable
                {
                    self.modals.push(ModalState::TextPrompt {
                        purpose: TextPromptPurpose::RenameCustomPack(p.id.clone()),
                        prompt: "Enter new pack title:".to_string(),
                        input: p.pack.meta.name.clone(),
                        max_len: 32,
                    });
                }
            }
            UiAction::HubDeletePack => {
                if let Some(p) = self.custom_packs.packs.get(self.custom_hub.selected_pack)
                    && p.writable
                {
                    self.modals.push(ModalState::Question {
                        id: QuestionId::ConfirmDeletePack(p.id.clone()),
                        message: format!(
                            "Delete custom pack '{}'?\n\nThis removes the XML file from disk.",
                            p.pack.meta.name
                        ),
                        yes_label: "yes",
                        no_label: Some("no"),
                    });
                }
            }
            UiAction::HubCloneEditable => {
                let Some(src) = self.custom_packs.packs.get(self.custom_hub.selected_pack) else {
                    return;
                };
                if src.writable {
                    return;
                }
                let mut cloned = src.pack.clone();
                cloned.meta.name = format!("{} (Copy)", cloned.meta.name);
                let new_id = self.unique_pack_id(&format!("{}-copy", src.id));
                match self.custom_packs.save_pack(&new_id, cloned) {
                    Ok(_) => {
                        self.select_pack_by_id(&new_id);
                        self.show_status_toast("Created editable copy of pack.");
                    }
                    Err(e) => self.show_status_toast(&e),
                }
            }
            UiAction::HubCycleDifficulty => {
                self.custom_hub.play_difficulty = match self.custom_hub.play_difficulty {
                    Difficulty::Easy => Difficulty::Medium,
                    Difficulty::Medium => Difficulty::Hard,
                    Difficulty::Hard => Difficulty::Easy,
                };
            }
            UiAction::HubPlayPack => {
                if let Some(p) = self.custom_packs.packs.get(self.custom_hub.selected_pack)
                    && !p.pack.levels.is_empty()
                {
                    let pid = p.id.clone();
                    let diff = self.custom_hub.play_difficulty;
                    self.launch_custom_pack_level(&pid, 0, diff, 0);
                }
            }
            UiAction::HubPlayLevel => {
                if let Some(p) = self.custom_packs.packs.get(self.custom_hub.selected_pack)
                    && self.custom_hub.selected_level < p.pack.levels.len()
                {
                    let pid = p.id.clone();
                    let idx = self.custom_hub.selected_level;
                    let diff = self.custom_hub.play_difficulty;
                    self.launch_custom_pack_level(&pid, idx, diff, 0);
                }
            }
            UiAction::HubEditLevel => {
                self.open_editor_for_selected_level();
            }
            UiAction::HubNewLevel => {
                let Some(entry) = self
                    .custom_packs
                    .packs
                    .get(self.custom_hub.selected_pack)
                    .cloned()
                else {
                    return;
                };
                if !entry.writable {
                    return;
                }
                let mut pack = entry.pack;
                let new_idx = pack.levels.len();
                pack.levels.push(default_starter_level(new_idx));
                if let Err(e) = self.custom_packs.save_pack(&entry.id, pack) {
                    self.show_status_toast(&e);
                    return;
                }
                self.select_pack_by_id(&entry.id);
                self.custom_hub.selected_level = new_idx;
                self.custom_hub.sync_pages_to_selection();
            }
            UiAction::HubGenerateLevel => {
                let Some(entry) = self
                    .custom_packs
                    .packs
                    .get(self.custom_hub.selected_pack)
                    .cloned()
                else {
                    return;
                };
                if !entry.writable {
                    return;
                }
                let gd = match self.custom_hub.play_difficulty {
                    Difficulty::Easy => GenDifficulty::Easy,
                    Difficulty::Medium => GenDifficulty::Medium,
                    Difficulty::Hard => GenDifficulty::Hard,
                };
                let mut lvl = generate_custom_level(gd, &mut self.rng);
                let mut pack = entry.pack;
                let new_idx = pack.levels.len();
                lvl.index = new_idx;
                pack.levels.push(lvl);
                if let Err(e) = self.custom_packs.save_pack(&entry.id, pack) {
                    self.show_status_toast(&e);
                    return;
                }
                self.select_pack_by_id(&entry.id);
                self.custom_hub.selected_level = new_idx;
                self.custom_hub.sync_pages_to_selection();
                self.show_status_toast("Generated verified solvable level!");
            }
            UiAction::HubDuplicateLevel => {
                let Some(entry) = self
                    .custom_packs
                    .packs
                    .get(self.custom_hub.selected_pack)
                    .cloned()
                else {
                    return;
                };
                if !entry.writable || self.custom_hub.selected_level >= entry.pack.levels.len() {
                    return;
                }
                let mut pack = entry.pack;
                let mut dup = pack.levels[self.custom_hub.selected_level].clone();
                dup.title = format!("{} (Copy)", dup.title);
                let insert_at = self.custom_hub.selected_level + 1;
                pack.levels.insert(insert_at, dup);
                for (i, l) in pack.levels.iter_mut().enumerate() {
                    l.index = i;
                }
                if let Err(e) = self.custom_packs.save_pack(&entry.id, pack) {
                    self.show_status_toast(&e);
                    return;
                }
                self.select_pack_by_id(&entry.id);
                self.custom_hub.selected_level = insert_at;
                self.custom_hub.sync_pages_to_selection();
            }
            UiAction::HubMoveLevelUp => {
                let idx = self.custom_hub.selected_level;
                if idx == 0 {
                    return;
                }
                let Some(entry) = self
                    .custom_packs
                    .packs
                    .get(self.custom_hub.selected_pack)
                    .cloned()
                else {
                    return;
                };
                if !entry.writable || idx >= entry.pack.levels.len() {
                    return;
                }
                let mut pack = entry.pack;
                pack.levels.swap(idx, idx - 1);
                for (i, l) in pack.levels.iter_mut().enumerate() {
                    l.index = i;
                }
                if let Err(e) = self.custom_packs.save_pack(&entry.id, pack) {
                    self.show_status_toast(&e);
                    return;
                }
                self.select_pack_by_id(&entry.id);
                self.custom_hub.selected_level = idx - 1;
                self.custom_hub.sync_pages_to_selection();
            }
            UiAction::HubMoveLevelDown => {
                let idx = self.custom_hub.selected_level;
                let Some(entry) = self
                    .custom_packs
                    .packs
                    .get(self.custom_hub.selected_pack)
                    .cloned()
                else {
                    return;
                };
                if !entry.writable || idx + 1 >= entry.pack.levels.len() {
                    return;
                }
                let mut pack = entry.pack;
                pack.levels.swap(idx, idx + 1);
                for (i, l) in pack.levels.iter_mut().enumerate() {
                    l.index = i;
                }
                if let Err(e) = self.custom_packs.save_pack(&entry.id, pack) {
                    self.show_status_toast(&e);
                    return;
                }
                self.select_pack_by_id(&entry.id);
                self.custom_hub.selected_level = idx + 1;
                self.custom_hub.sync_pages_to_selection();
            }
            UiAction::HubDeleteLevel => {
                let Some(entry) = self.custom_packs.packs.get(self.custom_hub.selected_pack) else {
                    return;
                };
                if !entry.writable || self.custom_hub.selected_level >= entry.pack.levels.len() {
                    return;
                }
                if entry.pack.levels.len() <= 1 {
                    self.show_status_toast("A pack must keep at least 1 level.");
                    return;
                }
                let lvl = &entry.pack.levels[self.custom_hub.selected_level];
                self.modals.push(ModalState::Question {
                    id: QuestionId::ConfirmDeleteCustomLevel {
                        pack_id: entry.id.clone(),
                        level_idx: self.custom_hub.selected_level,
                    },
                    message: format!(
                        "Delete level '{}' from '{}'?",
                        lvl.title, entry.pack.meta.name
                    ),
                    yes_label: "yes",
                    no_label: Some("no"),
                });
            }
            _ => {}
        }
    }

    pub fn launch_custom_pack_level(
        &mut self,
        pack_id: &str,
        level_idx: usize,
        difficulty: Difficulty,
        carry_score: u32,
    ) {
        let Some(def) = self
            .custom_packs
            .packs
            .iter()
            .find(|p| p.id == pack_id)
            .and_then(|p| p.pack.levels.get(level_idx))
            .cloned()
        else {
            self.show_status_toast("Custom level could not be loaded.");
            return;
        };
        self.fail_count = 0;
        self.temp_easy_revert = None;
        self.retry_backup = None;
        self.is_retrying_level = false;
        self.current_level_idx = level_idx;
        self.play_mode = PlayMode::CustomPack {
            pack_id: pack_id.to_string(),
            level_idx,
            session_score: carry_score,
            difficulty,
        };
        self.launch_level_def(def);
    }

    pub fn on_dropped_file(&mut self, path: &Path) {
        let is_xml = path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("xml"));
        if !is_xml {
            self.show_status_toast("Drop an Aqualux .xml file to import custom levels.");
            return;
        }
        match self.custom_packs.import_xml_file(path) {
            Ok(id) => {
                self.audio.play_sfx("BonusTwoSound");
                if self.screen == Screen::Title {
                    self.open_custom_hub(true);
                }
                self.select_pack_by_id(&id);
                self.show_status_toast(&format!("Imported pack '{}'!", id));
            }
            Err(e) => self.show_status_toast(&format!("Import failed: {}", e)),
        }
    }

    pub(crate) fn handle_submit_text_prompt(&mut self) {
        let Some(ModalState::TextPrompt { purpose, input, .. }) = self.modals.pop() else {
            return;
        };
        let trimmed = input.trim();
        match purpose {
            TextPromptPurpose::EditLevelDescription => {
                if let Some(ModalState::LevelProps(props)) = self.modals.last_mut() {
                    props.description = trimmed.to_string();
                }
            }
            TextPromptPurpose::EditLevelTitle => {
                if trimmed.is_empty() {
                    return;
                }
                if let Some(ModalState::LevelProps(props)) = self.modals.last_mut() {
                    props.title = trimmed.to_string();
                }
            }
            TextPromptPurpose::NewCustomPack => {
                if trimmed.is_empty() {
                    return;
                }
                let id = self.unique_pack_id(trimmed);
                let pack = CustomPackFile {
                    meta: CustomPackMeta {
                        name: trimmed.to_string(),
                        author: self
                            .save
                            .active_profile()
                            .map(|p| p.name.clone())
                            .unwrap_or_default(),
                        description: String::new(),
                    },
                    levels: vec![default_starter_level(0)],
                };
                match self.custom_packs.save_pack(&id, pack) {
                    Ok(_) => {
                        self.select_pack_by_id(&id);
                        self.show_status_toast(&format!("Created pack '{}'.", trimmed));
                    }
                    Err(e) => self.show_status_toast(&e),
                }
            }
            TextPromptPurpose::RenameCustomPack(pack_id) => {
                if trimmed.is_empty() {
                    return;
                }
                let Some(entry) = self
                    .custom_packs
                    .packs
                    .iter()
                    .find(|p| p.id == pack_id)
                    .cloned()
                else {
                    return;
                };
                if !entry.writable {
                    return;
                }
                let mut pack = entry.pack;
                pack.meta.name = trimmed.to_string();
                match self.custom_packs.save_pack(&pack_id, pack) {
                    Ok(_) => self.select_pack_by_id(&pack_id),
                    Err(e) => self.show_status_toast(&e),
                }
            }
        }
    }

    pub fn select_pack_by_id(&mut self, id: &str) {
        if let Some(pos) = self.custom_packs.packs.iter().position(|p| p.id == id) {
            self.custom_hub.selected_pack = pos;
            self.custom_hub.selected_level = 0;
            self.custom_hub.clamp_selection(&self.custom_packs);
            self.custom_hub.sync_pages_to_selection();
        }
    }

    fn unique_pack_id(&self, raw_name: &str) -> String {
        let base = sanitize_pack_id(raw_name);
        let mut candidate = base.clone();
        let mut n = 2usize;
        while self.custom_packs.packs.iter().any(|p| p.id == candidate) {
            candidate = format!("{}-{}", base, n);
            n += 1;
        }
        candidate
    }
}
