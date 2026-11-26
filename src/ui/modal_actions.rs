// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::AwardKind;
use crate::ui::summary::SummaryHideAction;
use crate::ui::title::{TitleMenuAnim, TitleProfileAnim, TitleSubmenu};
use crate::ui::widgets::{ModalState, QuestionId, UiAction};
use crate::ui::{GameApp, PlayMode};

impl GameApp {
    pub(crate) fn open_profile_modal(&mut self) {
        let items: Vec<String> = self.save.profiles.iter().map(|p| p.name.clone()).collect();
        let cur_idx = self
            .save
            .current_profile
            .as_deref()
            .and_then(|c| items.iter().position(|n| n == c))
            .unwrap_or(0);
        self.push_modal(ModalState::Profile {
            items,
            added: Vec::new(),
            selected: cur_idx,
            hover_slot: None,
        });
    }

    pub(crate) fn handle_profile_action(&mut self, action: UiAction) {
        match action {
            UiAction::ProfileSelectIdx(idx) => {
                if let Some(ModalState::Profile {
                    selected, items, ..
                }) = self.modals.last_mut()
                    && idx < items.len()
                {
                    *selected = idx;
                }
            }
            UiAction::ProfileNew => {
                if let Some(ModalState::Profile { items, .. }) = self.modals.last()
                    && items.len() >= 9
                {
                    self.push_modal(ModalState::Question {
                        id: QuestionId::ProfileFull,
                        message: "No slots left; delete a profile to add a new one.".to_string(),
                        yes_label: "ok",
                        no_label: None,
                    });
                    return;
                }
                self.push_modal(ModalState::EnterName {
                    prompt: "Enter your name:".to_string(),
                    input: String::new(),
                    allow_cancel: true,
                    creating_for_profile_list: true,
                    pending_launch_after: false,
                });
            }
            UiAction::ProfileDelete => {
                if let Some(ModalState::Profile {
                    items,
                    added,
                    selected,
                    ..
                }) = self.modals.last()
                {
                    if items.len() <= 1 {
                        return;
                    }
                    let target = items[*selected].clone();
                    let is_recent = added.iter().any(|a| a.eq_ignore_ascii_case(&target));
                    let msg = if is_recent {
                        format!("Really delete '{}'?", target)
                    } else {
                        format!(
                            "Really delete '{}'?\n \n  Saved progress will be lost.",
                            target
                        )
                    };
                    self.push_modal(ModalState::Question {
                        id: QuestionId::ConfirmDeleteProfile(target),
                        message: msg,
                        yes_label: "yes",
                        no_label: Some("no"),
                    });
                }
            }
            UiAction::ProfileAccept => {
                if let Some(ModalState::Profile {
                    items, selected, ..
                }) = self.pop_modal()
                {
                    if let Some(chosen) = items.get(selected) {
                        self.save.apply_profile_list(chosen, &items);
                    }
                    let is_unsaved = self
                        .save
                        .active_profile()
                        .is_none_or(|p| !p.has_saved_game());
                    self.title_wait_profile_latched = false;
                    self.title_profile_anim = Some(TitleProfileAnim::immediate(is_unsaved));
                    if self.title_submenu != TitleSubmenu::Main {
                        self.title_submenu = TitleSubmenu::Main;
                        self.title_menu_anim = TitleMenuAnim::FadeIn { elapsed: 0.0 };
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_submit_name(&mut self) {
        let Some(ModalState::EnterName {
            input,
            creating_for_profile_list,
            ..
        }) = self.modals.last_mut()
        else {
            return;
        };
        let trimmed = input.trim().to_string();
        if trimmed.is_empty() {
            *input = "Player 1".to_string();
            return;
        }
        let for_list = *creating_for_profile_list;
        if for_list {
            let duplicate = self.modals.iter().any(|m| {
                matches!(m, ModalState::Profile { items, .. } if items.iter().any(|i| i.eq_ignore_ascii_case(&trimmed)))
            });
            if duplicate {
                self.push_modal(ModalState::Question {
                    id: QuestionId::ProfileDuplicate,
                    message: format!("You already have a profile named '{}.'", trimmed),
                    yes_label: "ok",
                    no_label: None,
                });
                return;
            }
            self.audio.play_sfx("ClickOneSound");
            self.pop_modal();
            if let Some(ModalState::Profile {
                items,
                added,
                selected,
                ..
            }) = self.modals.last_mut()
            {
                items.push(trimmed.clone());
                added.push(trimmed);
                *selected = items.len().saturating_sub(1);
            }
        } else {
            self.audio.play_sfx("ClickOneSound");
            self.pop_modal();
            self.save.set_or_create_profile(&trimmed);
            self.title_wait_profile_latched = false;
            self.title_profile_anim = Some(TitleProfileAnim::on_first_profile_created());
        }
    }

    pub(crate) fn handle_question_yes(&mut self) {
        let Some(ModalState::Question { id, .. }) = self.pop_modal() else {
            return;
        };
        match id {
            QuestionId::ConfirmNewGame => {
                self.title_submenu = TitleSubmenu::DifficultyChoices;
                self.title_menu_anim = TitleMenuAnim::Instant;
            }
            QuestionId::ConfirmMainMenuFromPause => {
                self.return_to_main_menu();
            }
            QuestionId::ConfirmMainMenuFromSummary => {
                if self.play_mode == PlayMode::Campaign {
                    let len = self.assets.campaign_levels.len();
                    if let Some(prof) = self.save.active_profile_mut() {
                        prof.level_index = (self.current_level_idx + 1) % len.max(1);
                        if self.is_index_savable {
                            prof.savable_level_index = prof.level_index;
                        }
                    }
                }
                self.start_summary_hide(SummaryHideAction::MainMenu);
            }
            QuestionId::ConfirmDeletePack(pack_id) => {
                match self.custom_packs.delete_pack(&pack_id) {
                    Ok(()) => {
                        self.custom_hub.clamp_selection(&self.custom_packs);
                        self.show_status_toast("Deleted custom pack.");
                    }
                    Err(e) => self.show_status_toast(&e),
                }
            }
            QuestionId::ConfirmDeleteCustomLevel { pack_id, level_idx } => {
                let Some(entry) = self
                    .custom_packs
                    .packs
                    .iter()
                    .find(|p| p.id == pack_id)
                    .cloned()
                else {
                    return;
                };
                if entry.writable
                    && entry.pack.levels.len() > 1
                    && level_idx < entry.pack.levels.len()
                {
                    let mut pack = entry.pack;
                    pack.levels.remove(level_idx);
                    for (i, l) in pack.levels.iter_mut().enumerate() {
                        l.index = i;
                    }
                    match self.custom_packs.save_pack(&pack_id, pack) {
                        Ok(_) => {
                            self.select_pack_by_id(&pack_id);
                            self.custom_hub.selected_level = level_idx.saturating_sub(1);
                            self.custom_hub.clamp_selection(&self.custom_packs);
                            self.custom_hub.sync_pages_to_selection();
                        }
                        Err(e) => self.show_status_toast(&e),
                    }
                }
            }
            QuestionId::ConfirmDiscardEditor => {
                self.close_editor_to_hub();
            }
            QuestionId::ConfirmClearEditorBoard => {
                if let Some(ed) = &mut self.editor {
                    ed.clear_to_blank();
                    self.audio.play_sfx("SwapTwoSound");
                }
            }
            QuestionId::ConfirmDeleteProfile(name) => {
                if let Some(ModalState::Profile {
                    items, selected, ..
                }) = self.modals.last_mut()
                    && let Some(pos) = items.iter().position(|n| n == &name)
                {
                    items.remove(pos);
                    if *selected >= items.len() {
                        *selected = items.len().saturating_sub(1);
                    }
                }
            }
            QuestionId::ConfirmQuit => {
                self.save.save();
                self.should_quit = true;
            }
            QuestionId::UnlockMatchNotice(lvl) => {
                self.launch_level_def(*lvl);
            }
            QuestionId::UnlockMirrorNotice(need_ultra_award) => {
                self.audio.play_sfx("BonusTwoSound");
                self.push_modal(ModalState::Award {
                    kind: AwardKind::Champ,
                    then_champion: true,
                    then_ultralux: need_ultra_award,
                    then_unlock_match_level: None,
                });
            }
            QuestionId::TrophyNeedSave
            | QuestionId::ProfileFull
            | QuestionId::ProfileDuplicate
            | QuestionId::SimpleNotice => {}
        }
    }

    pub(crate) fn handle_dismiss_award(&mut self) {
        let Some(ModalState::Award {
            then_champion,
            then_ultralux,
            then_unlock_match_level,
            ..
        }) = self.pop_modal()
        else {
            return;
        };
        if let Some(lvl) = then_unlock_match_level {
            self.audio.play_sfx("BonusTwoSound");
            self.push_modal(ModalState::Question {
                id: QuestionId::UnlockMatchNotice(lvl),
                message: "-SECRET UNLOCKED!-\n\nMATCH MODE has been unlocked on the main menu!"
                    .to_string(),
                yes_label: "Ok",
                no_label: None,
            });
        } else if then_ultralux {
            self.audio.play_sfx("BonusTwoSound");
            self.push_modal(ModalState::Award {
                kind: AwardKind::Ultralux,
                then_champion,
                then_ultralux: false,
                then_unlock_match_level: None,
            });
        } else if then_champion {
            let score = self
                .save
                .active_profile()
                .map(|p| p.total_score())
                .unwrap_or(0);
            let has_u = self.save.active_profile().is_some_and(|p| p.awards[3]);
            self.enter_champion_screen(score, has_u);
        } else if let Some((res, old_total, new_total)) = self.pending_summary.take() {
            self.show_summary(res, old_total, new_total);
        } else if let Some(sim) = &self.match_sim {
            let is_mirror = sim.mirror_mode;
            let idx = self
                .save
                .active_profile()
                .map(|p| {
                    if is_mirror {
                        p.mirror_level_index.max(0) as usize
                    } else {
                        p.match_level_index.max(0) as usize
                    }
                })
                .unwrap_or(0);
            self.start_match_level(idx, is_mirror, false);
        }
    }
}
