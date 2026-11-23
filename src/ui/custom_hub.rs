// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::custom::CustomPackStore;
use crate::game::{Difficulty, Trophy};
use crate::render::{Canvas, FontFamily, FontSet, LOGICAL_H, LOGICAL_W, Rect, TexBank, TextAlign};
use crate::ui::widgets::{
    UiAction, UiButton, draw_modal_bubble, draw_modal_button, draw_text_button,
};

const PACKS_PER_PAGE: usize = 8;
const LEVELS_PER_PAGE: usize = 7;

#[derive(Debug, Clone)]
pub struct CustomHubState {
    pub play_focused: bool,
    pub selected_pack: usize,
    pub selected_level: usize,
    pub pack_page: usize,
    pub level_page: usize,
    pub play_difficulty: Difficulty,
}

impl Default for CustomHubState {
    fn default() -> Self {
        Self {
            play_focused: false,
            selected_pack: 0,
            selected_level: 0,
            pack_page: 0,
            level_page: 0,
            play_difficulty: Difficulty::Medium,
        }
    }
}

impl CustomHubState {
    pub fn clamp_selection(&mut self, store: &CustomPackStore) {
        if store.packs.is_empty() {
            self.selected_pack = 0;
            self.selected_level = 0;
            self.pack_page = 0;
            self.level_page = 0;
            return;
        }
        if self.selected_pack >= store.packs.len() {
            self.selected_pack = store.packs.len() - 1;
        }
        let max_pack_page = store.packs.len().saturating_sub(1) / PACKS_PER_PAGE;
        if self.pack_page > max_pack_page {
            self.pack_page = self.selected_pack / PACKS_PER_PAGE;
        }

        let lvl_len = store.packs[self.selected_pack].pack.levels.len();
        if lvl_len == 0 {
            self.selected_level = 0;
            self.level_page = 0;
        } else {
            if self.selected_level >= lvl_len {
                self.selected_level = lvl_len - 1;
            }
            let max_lvl_page = lvl_len.saturating_sub(1) / LEVELS_PER_PAGE;
            if self.level_page > max_lvl_page {
                self.level_page = self.selected_level / LEVELS_PER_PAGE;
            }
        }
    }

    pub fn sync_pages_to_selection(&mut self) {
        self.pack_page = self.selected_pack / PACKS_PER_PAGE;
        self.level_page = self.selected_level / LEVELS_PER_PAGE;
    }
}

pub fn build_custom_hub_buttons(store: &CustomPackStore, state: &CustomHubState) -> Vec<UiButton> {
    let mut out = vec![
        UiButton {
            id: "hub_folder",
            rect: Rect::new(506.0, 52.0, 140.0, 24.0),
            action: UiAction::HubOpenFolder,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "hub_refresh",
            rect: Rect::new(654.0, 52.0, 84.0, 24.0),
            action: UiAction::HubReloadPacks,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "hub_diff",
            rect: Rect::new(548.0, 86.0, 190.0, 24.0),
            action: UiAction::HubCycleDifficulty,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
    ];

    let pack_start = state.pack_page * PACKS_PER_PAGE;
    let pack_end = (pack_start + PACKS_PER_PAGE).min(store.packs.len());
    for (row, idx) in (pack_start..pack_end).enumerate() {
        out.push(UiButton {
            id: "hub_pack_item",
            rect: Rect::new(64.0, 116.0 + row as f32 * 35.0, 228.0, 33.0),
            action: UiAction::HubSelectPack(idx),
            enabled: true,
            hover_sfx: None,
        });
    }

    let total_pack_pages = store.packs.len().max(1).div_ceil(PACKS_PER_PAGE);
    if total_pack_pages > 1 {
        out.push(UiButton {
            id: "hub_pack_prev",
            rect: Rect::new(62.0, 412.0, 70.0, 22.0),
            action: UiAction::HubPackPrevPage,
            enabled: state.pack_page > 0,
            hover_sfx: Some("PopTwoSound"),
        });
        out.push(UiButton {
            id: "hub_pack_next",
            rect: Rect::new(224.0, 412.0, 70.0, 22.0),
            action: UiAction::HubPackNextPage,
            enabled: state.pack_page + 1 < total_pack_pages,
            hover_sfx: Some("PopTwoSound"),
        });
    }

    let sel_pack = store.packs.get(state.selected_pack);
    let writable = sel_pack.is_some_and(|p| p.writable);
    let has_ro_pack = sel_pack.is_some_and(|p| !p.writable);
    let lvl_count = sel_pack.map(|p| p.pack.levels.len()).unwrap_or(0);
    let has_level = lvl_count > 0 && state.selected_level < lvl_count;

    out.extend([
        UiButton {
            id: "hub_new_pack",
            rect: Rect::new(62.0, 440.0, 116.0, 22.0),
            action: UiAction::HubNewPack,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "hub_rename_pack",
            rect: Rect::new(186.0, 440.0, 108.0, 22.0),
            action: UiAction::HubRenamePack,
            enabled: writable,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "hub_del_pack",
            rect: Rect::new(62.0, 466.0, 116.0, 22.0),
            action: UiAction::HubDeletePack,
            enabled: writable,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "hub_clone_pack",
            rect: Rect::new(186.0, 466.0, 108.0, 22.0),
            action: UiAction::HubCloneEditable,
            enabled: has_ro_pack,
            hover_sfx: Some("PopTwoSound"),
        },
    ]);

    let lvl_start = state.level_page * LEVELS_PER_PAGE;
    let lvl_end = (lvl_start + LEVELS_PER_PAGE).min(lvl_count);
    for (row, idx) in (lvl_start..lvl_end).enumerate() {
        out.push(UiButton {
            id: "hub_lvl_item",
            rect: Rect::new(316.0, 116.0 + row as f32 * 40.0, 420.0, 38.0),
            action: UiAction::HubSelectLevel(idx),
            enabled: true,
            hover_sfx: None,
        });
    }

    let total_lvl_pages = lvl_count.max(1).div_ceil(LEVELS_PER_PAGE);
    if total_lvl_pages > 1 {
        out.push(UiButton {
            id: "hub_lvl_prev",
            rect: Rect::new(314.0, 412.0, 72.0, 22.0),
            action: UiAction::HubLevelPrevPage,
            enabled: state.level_page > 0,
            hover_sfx: Some("PopTwoSound"),
        });
        out.push(UiButton {
            id: "hub_lvl_next",
            rect: Rect::new(666.0, 412.0, 72.0, 22.0),
            action: UiAction::HubLevelNextPage,
            enabled: state.level_page + 1 < total_lvl_pages,
            hover_sfx: Some("PopTwoSound"),
        });
    }

    out.extend([
        UiButton {
            id: "hub_new_lvl",
            rect: Rect::new(314.0, 440.0, 136.0, 22.0),
            action: UiAction::HubNewLevel,
            enabled: writable,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "hub_gen_lvl",
            rect: Rect::new(456.0, 440.0, 136.0, 22.0),
            action: UiAction::HubGenerateLevel,
            enabled: writable,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "hub_dup_lvl",
            rect: Rect::new(598.0, 440.0, 140.0, 22.0),
            action: UiAction::HubDuplicateLevel,
            enabled: has_level && writable,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "hub_up_lvl",
            rect: Rect::new(314.0, 466.0, 136.0, 22.0),
            action: UiAction::HubMoveLevelUp,
            enabled: has_level && writable && state.selected_level > 0,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "hub_dn_lvl",
            rect: Rect::new(456.0, 466.0, 136.0, 22.0),
            action: UiAction::HubMoveLevelDown,
            enabled: has_level && writable && state.selected_level + 1 < lvl_count,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "hub_del_lvl",
            rect: Rect::new(598.0, 466.0, 140.0, 22.0),
            action: UiAction::HubDeleteLevel,
            enabled: has_level && writable,
            hover_sfx: Some("PopTwoSound"),
        },
        UiButton {
            id: "hub_play_lvl",
            rect: Rect::new(337.0, 498.0, 94.0, 53.0),
            action: UiAction::HubPlayLevel,
            enabled: has_level,
            hover_sfx: None,
        },
        UiButton {
            id: "hub_play_pack",
            rect: Rect::new(441.0, 498.0, 94.0, 53.0),
            action: UiAction::HubPlayPack,
            enabled: lvl_count > 0,
            hover_sfx: None,
        },
        UiButton {
            id: "hub_edit_lvl",
            rect: Rect::new(545.0, 498.0, 94.0, 53.0),
            action: UiAction::HubEditLevel,
            enabled: has_level && writable,
            hover_sfx: None,
        },
        UiButton {
            id: "hub_back",
            rect: Rect::new(649.0, 498.0, 94.0, 53.0),
            action: UiAction::HubBackToTitle,
            enabled: true,
            hover_sfx: None,
        },
    ]);

    out
}

pub fn render_custom_hub(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    store: &CustomPackStore,
    state: &CustomHubState,
    mouse_pos: (f32, f32),
) {
    canvas.fill_rect_alpha(
        Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
        0,
        0,
        0,
        105,
    );
    draw_modal_bubble(canvas, Rect::new(42.0, 34.0, 716.0, 532.0));

    let title = if state.play_focused {
        "-Custom Level Packs-"
    } else {
        "-Levelmaker-"
    };
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        62.0,
        49.0,
        24.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Left,
        title,
    );

    draw_link(
        canvas,
        fonts,
        Rect::new(506.0, 52.0, 140.0, 24.0),
        646.0,
        54.0,
        16.0,
        TextAlign::Right,
        "[Open Folder]",
        Some((0x42, 0x83, 0xF3)),
        true,
        mouse_pos,
    );
    draw_link(
        canvas,
        fonts,
        Rect::new(654.0, 52.0, 84.0, 24.0),
        738.0,
        54.0,
        16.0,
        TextAlign::Right,
        "[Refresh]",
        Some((0x42, 0x83, 0xF3)),
        true,
        mouse_pos,
    );

    // Left Column: Packs
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        62.0,
        86.0,
        18.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Left,
        "Packs",
    );
    canvas.draw_rounded_rect(
        Rect::new(62.0, 112.0, 232.0, 294.0),
        4.0,
        (0x05, 0x0D, 0x2B, 204),
        Some((0x0E, 0x15, 0x32, 255, 3.0)),
    );

    if store.packs.is_empty() {
        fonts.draw_multiline(
            canvas,
            FontFamily::Neuropol,
            178.0,
            210.0,
            15.0,
            20.0,
            (192, 208, 232),
            None,
            TextAlign::Center,
            200.0,
            "No packs found.\nClick '+ new pack' or drop an .xml file.",
            1.0,
        );
    } else {
        let pack_start = state.pack_page * PACKS_PER_PAGE;
        let pack_end = (pack_start + PACKS_PER_PAGE).min(store.packs.len());
        for (row, idx) in (pack_start..pack_end).enumerate() {
            let p = &store.packs[idx];
            let r = Rect::new(64.0, 116.0 + row as f32 * 35.0, 228.0, 33.0);
            if idx == state.selected_pack {
                canvas.fill_rect_alpha(r, 0x59, 0x95, 0xC1, 230);
            } else if r.contains(mouse_pos.0, mouse_pos.1) {
                canvas.fill_rect_alpha(r, 0x59, 0x95, 0xC1, 90);
            }
            let ro = if p.writable { "" } else { " [RO]" };
            let lbl = format!(
                "{} ({}){}",
                truncate_chars(&p.pack.meta.name, 14),
                p.pack.levels.len(),
                ro
            );
            fonts.draw(
                canvas,
                FontFamily::Neuropol,
                r.x + 8.0,
                r.y + 7.0,
                16.0,
                (255, 255, 255),
                TextAlign::Left,
                &lbl,
            );
        }
    }

    let total_pack_pages = store.packs.len().max(1).div_ceil(PACKS_PER_PAGE);
    if total_pack_pages > 1 {
        draw_link(
            canvas,
            fonts,
            Rect::new(62.0, 412.0, 70.0, 22.0),
            62.0,
            412.0,
            15.0,
            TextAlign::Left,
            "< prev",
            None,
            state.pack_page > 0,
            mouse_pos,
        );
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            178.0,
            413.0,
            14.0,
            (220, 232, 248),
            TextAlign::Center,
            &format!("{}/{}", state.pack_page + 1, total_pack_pages),
        );
        draw_link(
            canvas,
            fonts,
            Rect::new(224.0, 412.0, 70.0, 22.0),
            294.0,
            412.0,
            15.0,
            TextAlign::Right,
            "next >",
            None,
            state.pack_page + 1 < total_pack_pages,
            mouse_pos,
        );
    }

    let sel_pack = store.packs.get(state.selected_pack);
    let writable = sel_pack.is_some_and(|p| p.writable);
    let has_ro = sel_pack.is_some_and(|p| !p.writable);

    draw_link(
        canvas,
        fonts,
        Rect::new(62.0, 440.0, 116.0, 22.0),
        62.0,
        440.0,
        16.0,
        TextAlign::Left,
        "+ new pack",
        None,
        true,
        mouse_pos,
    );
    draw_link(
        canvas,
        fonts,
        Rect::new(186.0, 440.0, 108.0, 22.0),
        294.0,
        440.0,
        16.0,
        TextAlign::Right,
        "rename",
        None,
        writable,
        mouse_pos,
    );
    draw_link(
        canvas,
        fonts,
        Rect::new(62.0, 466.0, 116.0, 22.0),
        62.0,
        466.0,
        16.0,
        TextAlign::Left,
        "delete",
        None,
        writable,
        mouse_pos,
    );
    draw_link(
        canvas,
        fonts,
        Rect::new(186.0, 466.0, 108.0, 22.0),
        294.0,
        466.0,
        16.0,
        TextAlign::Right,
        "copy edit",
        None,
        has_ro,
        mouse_pos,
    );

    // Right Column: Levels
    let pack_heading = sel_pack
        .map(|p| truncate_chars(&p.pack.meta.name, 20))
        .unwrap_or_else(|| "Levels".to_string());
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        314.0,
        86.0,
        18.0,
        (255, 255, 255),
        (5, 13, 43),
        TextAlign::Left,
        &pack_heading,
    );
    draw_link(
        canvas,
        fonts,
        Rect::new(548.0, 86.0, 190.0, 24.0),
        738.0,
        87.0,
        16.0,
        TextAlign::Right,
        state.play_difficulty.human_label(),
        Some((0x42, 0x83, 0xF3)),
        true,
        mouse_pos,
    );

    canvas.draw_rounded_rect(
        Rect::new(314.0, 112.0, 424.0, 294.0),
        4.0,
        (0x05, 0x0D, 0x2B, 204),
        Some((0x0E, 0x15, 0x32, 255, 3.0)),
    );

    let lvl_count = sel_pack.map(|p| p.pack.levels.len()).unwrap_or(0);
    if let Some(pack_entry) = sel_pack {
        let lvl_start = state.level_page * LEVELS_PER_PAGE;
        let lvl_end = (lvl_start + LEVELS_PER_PAGE).min(lvl_count);
        let pack_prog = store.progress.packs.get(&pack_entry.id);
        for (row, idx) in (lvl_start..lvl_end).enumerate() {
            let lvl = &pack_entry.pack.levels[idx];
            let r = Rect::new(316.0, 116.0 + row as f32 * 40.0, 420.0, 38.0);
            if idx == state.selected_level {
                canvas.fill_rect_alpha(r, 0x59, 0x95, 0xC1, 230);
            } else if r.contains(mouse_pos.0, mouse_pos.1) {
                canvas.fill_rect_alpha(r, 0x59, 0x95, 0xC1, 90);
            }
            let title_str = format!("{}. {}", idx + 1, truncate_chars(&lvl.title, 20));
            fonts.draw(
                canvas,
                FontFamily::Neuropol,
                r.x + 8.0,
                r.y + 10.0,
                16.0,
                (255, 255, 255),
                TextAlign::Left,
                &title_str,
            );
            let meta_str = format!("{}x{}  Par {}", lvl.cols, lvl.rows, lvl.par_moves);
            fonts.draw(
                canvas,
                FontFamily::Neuropol,
                r.x + r.w - 42.0,
                r.y + 11.0,
                14.0,
                (0xC0, 0xD4, 0xF0),
                TextAlign::Right,
                &meta_str,
            );
            let trophy = pack_prog
                .and_then(|m| m.get(&idx))
                .map(|rec| rec.best_trophy)
                .unwrap_or(Trophy::None);
            if let Some((tid, _, _)) = bank.named(trophy.bitmap_name()) {
                canvas.draw_tex_rect(
                    tid,
                    Rect::new(r.x + r.w - 36.0, r.y + 4.0, 30.0, 30.0),
                    None,
                    1.0,
                    1.0,
                );
            }
        }
    }

    let total_lvl_pages = lvl_count.max(1).div_ceil(LEVELS_PER_PAGE);
    if total_lvl_pages > 1 {
        draw_link(
            canvas,
            fonts,
            Rect::new(314.0, 412.0, 72.0, 22.0),
            314.0,
            412.0,
            15.0,
            TextAlign::Left,
            "< prev",
            None,
            state.level_page > 0,
            mouse_pos,
        );
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            526.0,
            413.0,
            14.0,
            (220, 232, 248),
            TextAlign::Center,
            &format!("{}/{}", state.level_page + 1, total_lvl_pages),
        );
        draw_link(
            canvas,
            fonts,
            Rect::new(666.0, 412.0, 72.0, 22.0),
            738.0,
            412.0,
            15.0,
            TextAlign::Right,
            "next >",
            None,
            state.level_page + 1 < total_lvl_pages,
            mouse_pos,
        );
    }

    let has_level = lvl_count > 0 && state.selected_level < lvl_count;
    draw_link(
        canvas,
        fonts,
        Rect::new(314.0, 440.0, 136.0, 22.0),
        314.0,
        440.0,
        16.0,
        TextAlign::Left,
        "+ new level",
        None,
        writable,
        mouse_pos,
    );
    draw_link(
        canvas,
        fonts,
        Rect::new(456.0, 440.0, 136.0, 22.0),
        524.0,
        440.0,
        16.0,
        TextAlign::Center,
        "generate",
        None,
        writable,
        mouse_pos,
    );
    draw_link(
        canvas,
        fonts,
        Rect::new(598.0, 440.0, 140.0, 22.0),
        738.0,
        440.0,
        16.0,
        TextAlign::Right,
        "duplicate",
        None,
        has_level && writable,
        mouse_pos,
    );
    draw_link(
        canvas,
        fonts,
        Rect::new(314.0, 466.0, 136.0, 22.0),
        314.0,
        466.0,
        16.0,
        TextAlign::Left,
        "move up",
        None,
        has_level && writable && state.selected_level > 0,
        mouse_pos,
    );
    draw_link(
        canvas,
        fonts,
        Rect::new(456.0, 466.0, 136.0, 22.0),
        524.0,
        466.0,
        16.0,
        TextAlign::Center,
        "move down",
        None,
        has_level && writable && state.selected_level + 1 < lvl_count,
        mouse_pos,
    );
    draw_link(
        canvas,
        fonts,
        Rect::new(598.0, 466.0, 140.0, 22.0),
        738.0,
        466.0,
        16.0,
        TextAlign::Right,
        "delete level",
        None,
        has_level && writable,
        mouse_pos,
    );

    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        62.0,
        514.0,
        13.0,
        (0xC0, 0xD4, 0xF0),
        TextAlign::Left,
        "Drop .xml file here to import",
    );

    for (r, label, en) in [
        (Rect::new(337.0, 498.0, 94.0, 53.0), "play", has_level),
        (Rect::new(441.0, 498.0, 94.0, 53.0), "pack", lvl_count > 0),
        (
            Rect::new(545.0, 498.0, 94.0, 53.0),
            "edit",
            has_level && writable,
        ),
        (Rect::new(649.0, 498.0, 94.0, 53.0), "done", true),
    ] {
        draw_modal_button(
            canvas,
            bank,
            fonts,
            r,
            label,
            en && r.contains(mouse_pos.0, mouse_pos.1),
            en,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_link(
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

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let head: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", head)
    }
}
