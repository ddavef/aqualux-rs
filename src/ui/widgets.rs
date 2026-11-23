// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::{AwardKind, Difficulty, GameColor, LevelDef};
use crate::render::{Canvas, FontFamily, FontSet, GlowPreset, Rect, TexBank, TextAlign};
use crate::ui::editor::{EditorTool, LevelPropsDraft, PipeBehavior, PipeColorSlot};
pub use crate::ui::roller::ScoreRoller;

#[derive(Debug, Clone, PartialEq)]
pub enum QuestionId {
    ConfirmNewGame,
    ConfirmMainMenuFromPause,
    ConfirmMainMenuFromSummary,
    ConfirmDeleteProfile(String),
    ConfirmDeletePack(String),
    ConfirmDeleteCustomLevel { pack_id: String, level_idx: usize },
    ConfirmDiscardEditor,
    ConfirmClearEditorBoard,
    ConfirmQuit,
    TrophyNeedSave,
    ProfileFull,
    ProfileDuplicate,
    UnlockMatchNotice(Box<LevelDef>),
    UnlockMirrorNotice(bool),
    SimpleNotice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextPromptPurpose {
    NewCustomPack,
    RenameCustomPack(String),
    EditLevelTitle,
    EditLevelDescription,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchSource {
    NewCampaign(Difficulty),
    ContinueCampaign,
    Lotd,
    CustomHub { play_focused: bool },
    Match { mirror: bool },
    RetryTrophy { idx: usize, difficulty: Difficulty },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModalState {
    TrophyRoom {
        page: usize,
        hover_idx: Option<usize>,
    },
    Options {
        allow_difficulty: bool,
    },
    Difficulty {
        selected: Difficulty,
        ok_only: bool,
        trophy_level_idx: Option<usize>,
    },
    InGameDifficulty {
        level_only: bool,
    },
    Profile {
        items: Vec<String>,
        added: Vec<String>,
        selected: usize,
        hover_slot: Option<usize>,
    },
    EnterName {
        prompt: String,
        input: String,
        allow_cancel: bool,
        creating_for_profile_list: bool,
        pending_launch_after: bool,
    },
    TextPrompt {
        purpose: TextPromptPurpose,
        prompt: String,
        input: String,
        max_len: usize,
    },
    Question {
        id: QuestionId,
        message: String,
        yes_label: &'static str,
        no_label: Option<&'static str>,
    },
    Award {
        kind: AwardKind,
        then_champion: bool,
        then_ultralux: bool,
        then_unlock_match_level: Option<Box<LevelDef>>,
    },
    LevelProps(LevelPropsDraft),
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    // Title menu
    OpenPlayMenu,
    BackToMainMenu,
    BackToPlayMenu,
    StartOverPrompt,
    ConfirmDifficultyCampaign(Difficulty),
    ContinueCampaign,
    PlayLotd,
    StartMatch,
    StartMirrorMatch,
    OpenCustomHub { play_focused: bool },
    OpenTrophyRoom,
    OpenOptions,
    OpenProfileModal,
    OpenMmgNotice,
    QuitPrompt,

    // Custom Packs Hub
    SubmitTextPrompt,
    HubBackToTitle,
    HubOpenFolder,
    HubReloadPacks,
    HubSelectPack(usize),
    HubPackPrevPage,
    HubPackNextPage,
    HubNewPack,
    HubRenamePack,
    HubDeletePack,
    HubCloneEditable,
    HubCycleDifficulty,
    HubPlayPack,
    HubSelectLevel(usize),
    HubLevelPrevPage,
    HubLevelNextPage,
    HubPlayLevel,
    HubEditLevel,
    HubNewLevel,
    HubGenerateLevel,
    HubDuplicateLevel,
    HubMoveLevelUp,
    HubMoveLevelDown,
    HubDeleteLevel,

    // Level Editor & Properties
    EditorSelectTool(EditorTool),
    EditorRotateBrushCw,
    EditorSetBehavior(PipeBehavior),
    EditorToggleFreeze,
    EditorToggleGate,
    EditorToggleMagic,
    EditorSetPortalColor(GameColor),
    EditorResizeCols(i8),
    EditorResizeRows(i8),
    EditorUndo,
    EditorRedo,
    EditorClearBoard,
    EditorAutoPar,
    EditorToggleSolutionPath,
    EditorOpenProps,
    EditorVerify,
    EditorTest,
    EditorSave,
    EditorBack,
    PropsEditTitle,
    PropsEditDesc,
    PropsCycleBg,
    PropsCycleLiquid,
    PropsAdjustDelay(i8),
    PropsAdjustSpeed(i8),
    PropsCyclePipeColor(PipeColorSlot),
    PropsAutoPar,
    PropsAdjustParMoves(i32),
    PropsAdjustParPipes(i32),
    PropsAdjustParTimeSec(i32),
    PropsCycleTestDiff,
    PropsApply,

    // Playing & Pause
    PauseGame,
    ResumeGame,
    FastForward,
    NextTutorial,
    SkipTutorials,
    MainMenuPrompt,

    // Summary & Champion
    NextLevel,
    RetryLevel,
    MainMenuConfirmFromSummary,
    DirectMainMenu,
    DirectQuit,

    // Options
    SetMusicVol(f32),
    SetSfxVol(f32),
    ToggleColorBlind,
    ToggleWindowed,
    CycleFps,
    OpenOptionsDifficulty,
    CloseTopModal,

    // Difficulty modal
    SelectDifficultyRadio(Difficulty),
    ConfirmDifficultyModal,

    // Trophy room
    TrophyNextPage,
    TrophyPickLevel(usize),

    // Profile & Name modals
    ProfileSelectIdx(usize),
    ProfileNew,
    ProfileDelete,
    ProfileAccept,
    SubmitName,

    // In-game difficulty prompt & Question & Award
    ToggleLevelOnlyCheckbox,
    InGameDiffYes,
    InGameDiffNo,
    QuestionYes,
    QuestionNo,
    DismissAward,
}

#[derive(Debug, Clone)]
pub struct UiButton {
    pub id: &'static str,
    pub rect: Rect,
    pub action: UiAction,
    pub enabled: bool,
    pub hover_sfx: Option<&'static str>,
}

/// Renders beveled blue modal bubble (`0x1C4A84`, radius 35, 4px black border).
pub fn draw_modal_bubble(canvas: &mut Canvas, rect: Rect) {
    canvas.draw_modal_bubble(rect);
}

/// Renders `ModalButtonBitmap` (94x53) with white button hover glow (`0xFFFFFF`, blur 8) and `NEUROPOL` 20px white label.
pub fn draw_modal_button(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    rect: Rect,
    label: &str,
    hovered: bool,
    enabled: bool,
) {
    let alpha = if !enabled { 0.45 } else { 1.0 };
    if let Some((tid, bw, bh)) = bank.named("ModalButtonBitmap") {
        if hovered
            && enabled
            && let Some((gid, gw, gh, pad)) =
                bank.glow_named("ModalButtonBitmap", GlowPreset::ButtonWhite)
        {
            let sx = rect.w / bw.max(1) as f32;
            let sy = rect.h / bh.max(1) as f32;
            canvas.draw_tex_rect(
                gid,
                Rect::new(
                    rect.x - pad as f32 * sx,
                    rect.y - pad as f32 * sy,
                    gw as f32 * sx,
                    gh as f32 * sy,
                ),
                None,
                1.0,
                1.0,
            );
        }
        canvas.draw_tex_rect(tid, rect, None, 1.0, alpha);
    }
    let col = if enabled {
        (255, 255, 255)
    } else {
        (160, 170, 185)
    };
    fonts.draw_stroked(
        canvas,
        FontFamily::Neuropol,
        rect.x + rect.w * 0.5,
        rect.y + rect.h * 0.5 - 12.0,
        20.0,
        col,
        (0x05, 0x0D, 0x2B),
        TextAlign::Center,
        label,
    );
}

/// Renders interactive text button: `#20A1F2` idle, `#C9DA41` hovered, `#050D2B` stroke.
#[allow(clippy::too_many_arguments)]
pub fn draw_text_button_blurred(
    canvas: &mut Canvas,
    fonts: &FontSet,
    x: f32,
    y: f32,
    size: f32,
    align: TextAlign,
    label: &str,
    hovered: bool,
    idle_col: Option<(u8, u8, u8)>,
    alpha: f32,
    blur: (f32, f32),
) {
    let col = if hovered {
        (0xC9, 0xDA, 0x41)
    } else {
        idle_col.unwrap_or((0x20, 0xA1, 0xF2))
    };
    fonts.draw_fx(
        canvas,
        FontFamily::Neuropol,
        x,
        y,
        size,
        1.0,
        1.0,
        1.0,
        1.0,
        col,
        Some((0x05, 0x0D, 0x2B)),
        align,
        label,
        1.0,
        alpha,
        [1.0; 3],
        blur,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn draw_text_button(
    canvas: &mut Canvas,
    fonts: &FontSet,
    x: f32,
    y: f32,
    size: f32,
    align: TextAlign,
    label: &str,
    hovered: bool,
    idle_col: Option<(u8, u8, u8)>,
    alpha: f32,
) {
    draw_text_button_blurred(
        canvas,
        fonts,
        x,
        y,
        size,
        align,
        label,
        hovered,
        idle_col,
        alpha,
        (0.0, 0.0),
    );
}

/// Renders sharp 15x15 dark checkbox, 2px white stroke, X mark when checked, `NEUROPOL` 17px white label.
pub fn draw_checkbox(
    canvas: &mut Canvas,
    fonts: &FontSet,
    x: f32,
    y: f32,
    label: &str,
    checked: bool,
    _hovered: bool,
) {
    canvas.draw_rounded_rect(
        Rect::new(x, y, 15.0, 15.0),
        0.0,
        (0, 0, 0, 102),
        Some((255, 255, 255, 255, 2.0)),
    );
    if checked {
        canvas.draw_line(
            x + 1.0,
            y + 1.0,
            x + 14.0,
            y + 14.0,
            2.0,
            (255, 255, 255, 255),
        );
        canvas.draw_line(
            x + 14.0,
            y + 1.0,
            x + 1.0,
            y + 14.0,
            2.0,
            (255, 255, 255, 255),
        );
    }
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        x + 30.0,
        y - 3.0,
        17.0,
        (255, 255, 255),
        TextAlign::Left,
        label,
    );
}

/// Renders 120px `#4283F3` volume slider track with draggable `#7EAEF1` rounded nub.
pub fn draw_slider(canvas: &mut Canvas, x: f32, y: f32, value_01: f32) {
    let w = 120.0;
    let h = 20.0;
    let cy = y + h * 0.5;
    canvas.draw_line(x, cy, x + w, cy, 4.0, (0x42, 0x83, 0xF3, 255));
    let nx = x + value_01.clamp(0.0, 1.0) * w;
    let nw = 12.0;
    let nh = 20.0;
    canvas.draw_rounded_rect(
        Rect::new(nx - nw * 0.5, cy - nh * 0.5, nw, nh),
        4.0,
        (0x7E, 0xAE, 0xF1, 255),
        Some((255, 255, 255, 255, 2.0)),
    );
}

/// Renders radio selector: 1.5px white circle at `(8, 12)` + inner 4px dot + `NEUROPOL` 20px white label.
pub fn draw_radio(canvas: &mut Canvas, fonts: &FontSet, x: f32, y: f32, label: &str, active: bool) {
    let cx = x + 8.0;
    let cy = y + 12.0;
    canvas.stroke_circle_alpha(cx, cy, 8.0, 1.5, (255, 255, 255, 255));
    if active {
        canvas.fill_circle_alpha(cx, cy, 4.0, (255, 255, 255, 215));
    }
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        x + 25.0,
        y,
        20.0,
        (255, 255, 255),
        TextAlign::Left,
        label,
    );
}
