// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::AssetBundle;
use crate::game::scoring::{calculate_score, generate_match_level};
use crate::game::types::{LevelDef, TileDef};
use crate::game::{Difficulty, FlowTimerState, GameColor, LevelSim, PieceShape, PieceType, Trophy};
use crate::render::pipe_mask::draw_flow_timer;
use crate::render::{BoardLayout, Canvas, Renderer, ScoreBlurbFx, draw_score_blurbs};
use crate::save::{LevelRecord, ProfileData, SaveData};
use crate::tests::{asset_file_if_present, test_gpu};
use crate::ui::champion::{ChampionState, build_champion_buttons, render_champion_screen};
use crate::ui::hud;
use crate::ui::summary::{SummaryState, build_summary_buttons, render_summary_screen};
use crate::ui::title_data::{TitleSubmenu, menu_entries};
use crate::ui::widgets::{LaunchSource, ModalState, UiAction};
use crate::ui::{GameApp, PlayMode, Screen};
use rand::SeedableRng;
use rand::rngs::StdRng;

fn source_timer_remaining(app: &GameApp) -> Option<f32> {
    let sim = app.campaign_sim.as_ref()?;
    sim.tiles
        .iter()
        .flatten()
        .find(|t| t.piece_type == PieceType::Source)
        .and_then(|t| t.timer.as_ref().map(|tm| tm.remaining))
}

#[test]
fn options_difficulty_visibility_and_lotd_medium_parity() {
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let gpu = test_gpu();
    let mut app = GameApp::new(bundle, Some(120), gpu).expect("GameApp init");
    app.save.profiles = vec![ProfileData {
        name: "Tester".into(),
        ..Default::default()
    }];
    app.save.current_profile = Some("Tester".into());
    app.screen = Screen::Title;

    // Unsaved profile on Title: difficulty button is hidden.
    app.trigger_action(UiAction::OpenOptions);
    assert_eq!(
        app.modals.last(),
        Some(&ModalState::Options {
            allow_difficulty: false
        })
    );
    app.modals.clear();

    // Saved campaign on Title: difficulty button visible and edits savable/current difficulty.
    if let Some(p) = app.save.active_profile_mut() {
        p.records.insert(
            0,
            LevelRecord {
                best_trophy: Trophy::Gold,
                best_score: 1000,
                best_moves: Some(4),
                best_time_ms: Some(5000),
            },
        );
    }
    app.trigger_action(UiAction::OpenOptions);
    assert_eq!(
        app.modals.last(),
        Some(&ModalState::Options {
            allow_difficulty: true
        })
    );
    app.trigger_action(UiAction::OpenOptionsDifficulty);
    assert_eq!(
        app.modals.last(),
        Some(&ModalState::Difficulty {
            selected: Difficulty::Medium,
            ok_only: true,
            trophy_level_idx: None
        })
    );
    app.trigger_action(UiAction::SelectDifficultyRadio(Difficulty::Hard));
    app.trigger_action(UiAction::ConfirmDifficultyModal);
    assert_eq!(
        app.modals.last(),
        Some(&ModalState::Options {
            allow_difficulty: true
        })
    );
    assert_eq!(
        app.save.active_profile().unwrap().difficulty,
        Difficulty::Hard
    );
    assert_eq!(
        app.save.active_profile().unwrap().savable_difficulty,
        Difficulty::Hard
    );
    app.modals.clear();

    // Pause menu Options shows difficulty in Campaign, hides in Match and LOTD.
    app.execute_launch(LaunchSource::ContinueCampaign);
    app.trigger_action(UiAction::PauseGame);
    app.trigger_action(UiAction::OpenOptions);
    assert_eq!(
        app.modals.last(),
        Some(&ModalState::Options {
            allow_difficulty: true
        })
    );
    app.modals.clear();

    app.start_match_level(0, false, true);
    app.trigger_action(UiAction::PauseGame);
    app.trigger_action(UiAction::OpenOptions);
    assert_eq!(
        app.modals.last(),
        Some(&ModalState::Options {
            allow_difficulty: false
        })
    );
    app.modals.clear();

    // Cached LOTD levels always run at Medium difficulty regardless of profile difficulty.
    for camp_diff in [Difficulty::Easy, Difficulty::Hard] {
        if let Some(p) = app.save.active_profile_mut() {
            p.difficulty = camp_diff;
            p.savable_difficulty = camp_diff;
        }
        app.screen = Screen::Title;
        app.execute_launch(LaunchSource::Lotd);
        assert_eq!(app.play_mode, PlayMode::Lotd);
        {
            let sim = app.campaign_sim.as_mut().unwrap();
            assert_eq!(sim.level.title, "Think Fast");
            assert_eq!(sim.difficulty, Difficulty::Medium);
            assert!((sim.current_speed - 2.0).abs() < 1e-4);
            sim.start_level();
        }
        assert!((source_timer_remaining(&app).unwrap() - 75.0).abs() < 1e-4);

        app.trigger_action(UiAction::PauseGame);
        app.trigger_action(UiAction::OpenOptions);
        assert_eq!(
            app.modals.last(),
            Some(&ModalState::Options {
                allow_difficulty: false
            })
        );
        app.modals.clear();

        app.advance_after_summary();
        {
            let sim = app.campaign_sim.as_mut().unwrap();
            assert_eq!(sim.level.title, "the shortcut");
            assert_eq!(sim.difficulty, Difficulty::Medium);
            assert!((sim.current_speed - 15.0).abs() < 1e-4);
            sim.start_level();
        }
        assert!((source_timer_remaining(&app).unwrap() - 24.0).abs() < 1e-4);

        app.return_to_main_menu();
        assert_eq!(app.play_mode, PlayMode::Campaign);
        let prof = app.save.active_profile().unwrap();
        assert_eq!(prof.difficulty, camp_diff);
        assert_eq!(prof.savable_difficulty, camp_diff);
    }

    // Trophy Room retry must not clobber saved campaign index/difficulty on return to menu.
    if let Some(p) = app.save.active_profile_mut() {
        p.difficulty = Difficulty::Medium;
        p.savable_difficulty = Difficulty::Medium;
        p.level_index = 5;
        p.savable_level_index = 5;
    }
    app.execute_launch(LaunchSource::RetryTrophy {
        idx: 0,
        difficulty: Difficulty::Hard,
    });
    assert_eq!(app.current_level_idx, 0);
    assert_eq!(
        app.campaign_sim.as_ref().unwrap().difficulty,
        Difficulty::Hard
    );
    assert_eq!(app.save.active_profile().unwrap().savable_level_index, 5);
    assert_eq!(
        app.save.active_profile().unwrap().savable_difficulty,
        Difficulty::Medium
    );
    app.return_to_main_menu();
    let prof = app.save.active_profile().unwrap();
    assert_eq!(prof.difficulty, Difficulty::Medium);
    assert_eq!(prof.level_index, 5);
}

#[test]
fn frozen_pipe_slows_normal_flow_8x_but_not_fast_forward() {
    let make_def = |freeze: bool| LevelDef {
        index: 0,
        title: "Freeze".into(),
        description: String::new(),
        background_id: "SnowyHills".into(),
        liquid_color: GameColor::Blue,
        start_delay: 0.0,
        liquid_speed: 24.0,
        rotater_color: GameColor::Green,
        mover_color: GameColor::Orange,
        stationary_color: GameColor::Grey,
        gate_color: GameColor::White,
        freeze_color: GameColor::Blue,
        magic_color: GameColor::Violet,
        par_moves: 1,
        par_pipes: 3,
        par_time_ms: 5000,
        cols: 3,
        rows: 1,
        tiles: vec![
            TileDef {
                x: 0,
                y: 0,
                shape: PieceShape::Spigot,
                piece_type: PieceType::Source,
                direction: 3,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
            TileDef {
                x: 1,
                y: 0,
                shape: PieceShape::Straight,
                piece_type: PieceType::Pipe,
                direction: 1,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: freeze,
                is_magic: false,
                secondary_color: None,
            },
            TileDef {
                x: 2,
                y: 0,
                shape: PieceShape::Drain,
                piece_type: PieceType::Drain,
                direction: 1,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
        ],
        tutorials: Vec::new(),
    };
    let mut norm = LevelSim::new(make_def(false), Difficulty::Medium);
    let mut froz = LevelSim::new(make_def(true), Difficulty::Medium);
    norm.start_level();
    froz.start_level();

    let dt = 1.0 / 60.0;
    for _ in 0..300 {
        norm.update(dt);
        froz.update(dt);
        if !norm.tiles[0][1].fills.is_empty() && !froz.tiles[0][1].fills.is_empty() {
            break;
        }
    }
    let p0_n = norm.tiles[0][1].fills[0].progress;
    let p0_f = froz.tiles[0][1].fills[0].progress;
    for _ in 0..30 {
        norm.update(dt);
        froz.update(dt);
    }
    let dn = norm.tiles[0][1].fills[0].progress - p0_n;
    let df = froz.tiles[0][1].fills[0].progress - p0_f;
    assert!(
        dn > 0.1 && (dn / df - 8.0).abs() < 0.05,
        "Expected 8x slowdown, got {}/{} = {}",
        dn,
        df,
        dn / df
    );

    // Fast-forward raises current_speed to 120.0, bypassing the frozen pipe divisor.
    let _ = norm.fast_forward();
    let _ = froz.fast_forward();
    let pn_before = norm.tiles[0][1].fills[0].progress;
    let pf_before = froz.tiles[0][1].fills[0].progress;
    norm.update(dt);
    froz.update(dt);
    let fdn = norm.tiles[0][1].fills[0].progress - pn_before;
    let fdf = froz.tiles[0][1].fills[0].progress - pf_before;
    assert!(
        (fdn - fdf).abs() < 1e-4 && (fdf - 0.06).abs() < 1e-4,
        "Fast-forward mismatch: {} vs {}",
        fdn,
        fdf
    );
}

#[test]
fn animated_text_and_counters_do_not_thrash_font_cache() {
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let gpu = test_gpu();
    let renderer = Renderer::new(gpu, &bundle).expect("renderer");
    let mut canvas = Canvas::new();
    let mut rng = StdRng::seed_from_u64(11);
    let mut sum = SummaryState::new(calculate_score(4, 5, 6000, 4, 5, 8000, true), 1000, 9999);
    let mut champ = ChampionState::new(987_654, false, &mut rng);

    let mut run_frame = |frame: u32, sum_age: f32, champ_age: f32, remaining: f32| {
        let dt = 1.0 / 60.0;
        let t = frame as f32 * dt;
        sum.age = sum_age;
        champ.age = champ_age;
        canvas.begin_frame(1.5);
        let timer = FlowTimerState {
            total: 10.0,
            remaining,
            age: t.min(1.5),
            last_alarm_sec: 3,
            warning_red: remaining < 3.0,
            warn_elapsed: (remaining <= 3.0).then_some(3.0 - remaining),
            fading: false,
            fade_elapsed: 0.0,
        };
        draw_flow_timer(
            &mut canvas,
            &renderer.bank,
            &renderer.fonts,
            120.0,
            120.0,
            &timer,
            1.0,
        );
        let blurb = ScoreBlurbFx {
            x: 400.0,
            y: 300.0,
            text: "100".into(),
            age: (frame % 54) as f32 * dt,
        };
        draw_score_blurbs(&mut canvas, &renderer.fonts, &[blurb]);
        let sim = LevelSim::new(bundle.campaign_levels[0].clone(), Difficulty::Medium);
        let layout =
            BoardLayout::compute(sim.level.cols, sim.level.rows, hud::campaign_board_area());
        hud::render_campaign_hud(
            &mut canvas,
            &renderer.bank,
            &renderer.fonts,
            &sim,
            layout,
            12_345 + frame * 7,
            sim.moves,
            sim.elapsed_ms as u32,
            1.0,
            Some(1.0),
            (0.0, 0.0),
            false,
            true,
            &hud::HudAnimState::fully_visible(),
            true,
        );
        render_summary_screen(
            &mut canvas,
            &renderer.bank,
            &renderer.fonts,
            &sum,
            (0.0, 0.0),
        );
        render_champion_screen(
            &mut canvas,
            &renderer.bank,
            &renderer.fonts,
            &champ,
            (0.0, 0.0),
        );
    };

    for f in 0..80 {
        run_frame(
            f,
            0.5 + f as f32 * 0.05,
            3.0 + f as f32 * 0.08,
            12.0 - (f % 12) as f32,
        );
    }
    let after_warmup = renderer.fonts.cache_entry_count();
    for f in 80..200 {
        run_frame(
            f,
            1.0 + (f % 80) as f32 * 0.04,
            3.8 + (f % 80) as f32 * 0.06,
            2.8 - (f % 50) as f32 * 0.05,
        );
    }
    let after_anim = renderer.fonts.cache_entry_count();
    assert!(
        after_warmup < 95,
        "Expected compact glyph/label cache, got {}",
        after_warmup
    );
    assert_eq!(
        after_anim, after_warmup,
        "Animated frames must not allocate new baked font textures"
    );
}

#[test]
fn match_rotator_mirror_banner_and_end_screen_hide_parity() {
    let mut rng = StdRng::seed_from_u64(3);
    assert_eq!(
        generate_match_level(0, &mut rng).rotater_color,
        GameColor::White
    );

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let mut app = GameApp::new(bundle, Some(120), test_gpu()).expect("GameApp init");
    app.start_match_level(0, true, true);
    assert_eq!(app.info_banner.as_ref().unwrap().title, "Match Mode");

    app.start_campaign_level(0, false);
    app.board_pop_elapsed = None;
    let mut sum = SummaryState::new(calculate_score(4, 5, 6000, 4, 5, 8000, true), 1000, 2400);
    sum.age = 4.0;
    assert!(!build_summary_buttons(&sum).is_empty());
    app.summary_state = Some(sum);
    app.screen = Screen::Summary;

    app.trigger_action(UiAction::NextLevel);
    assert!(build_summary_buttons(app.summary_state.as_ref().unwrap()).is_empty());
    for _ in 0..20 {
        app.update(0.05);
    }
    assert_eq!(app.screen, Screen::Summary);
    for _ in 0..25 {
        app.update(0.05);
    }
    assert_eq!(app.screen, Screen::Playing);
    assert_eq!(app.current_level_idx, 1);

    app.enter_champion_screen(50_000, true);
    app.champion_state.as_mut().unwrap().age = 9.0;
    assert!(!build_champion_buttons(app.champion_state.as_ref().unwrap()).is_empty());
    app.trigger_action(UiAction::DirectMainMenu);
    assert_eq!(app.screen, Screen::Title);
    assert!(app.champion_state.is_some());
    assert!(app.current_buttons().is_empty());
    for _ in 0..10 {
        app.update(0.05);
    }
    assert!(app.champion_state.is_some());
    for _ in 0..15 {
        app.update(0.05);
    }
    assert_eq!(app.screen, Screen::Title);
    assert!(app.champion_state.is_none());
}

#[test]
fn both_match_and_mirror_modes_available_independently() {
    let mut save = SaveData {
        current_profile: Some("Test".to_string()),
        profiles: vec![ProfileData {
            name: "Test".to_string(),
            awards: [true, false, false, false],
            match_level_index: 2,
            mirror_level_index: -1,
            ..Default::default()
        }],
        ..Default::default()
    };

    let match_only = menu_entries(TitleSubmenu::PlayChoices, &save);
    assert!(
        match_only
            .iter()
            .any(|e| e.id == "btn_match" && e.action == Some(UiAction::StartMatch))
    );
    assert!(!match_only.iter().any(|e| e.id == "btn_mirror"));

    save.profiles[0].awards[1] = true;
    save.profiles[0].mirror_level_index = 5;
    let both = menu_entries(TitleSubmenu::PlayChoices, &save);
    assert!(
        both.iter()
            .any(|e| e.id == "btn_match" && e.action == Some(UiAction::StartMatch))
    );
    assert!(
        both.iter()
            .any(|e| e.id == "btn_mirror" && e.action == Some(UiAction::StartMirrorMatch))
    );

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let mut app = GameApp::new(bundle, Some(120), test_gpu()).expect("GameApp init");
    app.save = save;

    app.execute_launch(LaunchSource::Match { mirror: false });
    {
        let sim = app.match_sim.as_ref().expect("match sim active");
        assert!(!sim.mirror_mode);
        assert_eq!(sim.level_index, 2);
    }
    app.advance_after_match();
    {
        let prof = app.save.active_profile().unwrap();
        assert_eq!(prof.match_level_index, 3);
        assert_eq!(prof.mirror_level_index, 5);
        let sim = app.match_sim.as_ref().expect("next match sim active");
        assert!(!sim.mirror_mode);
        assert_eq!(sim.level_index, 3);
    }

    app.execute_launch(LaunchSource::Match { mirror: true });
    {
        let sim = app.match_sim.as_ref().expect("mirror sim active");
        assert!(sim.mirror_mode);
        assert_eq!(sim.level_index, 5);
    }
    app.advance_after_match();
    {
        let prof = app.save.active_profile().unwrap();
        assert_eq!(prof.match_level_index, 3);
        assert_eq!(prof.mirror_level_index, 6);
        let sim = app.match_sim.as_ref().expect("next mirror sim active");
        assert!(sim.mirror_mode);
        assert_eq!(sim.level_index, 6);
    }
}

#[test]
fn unlock_chains_retries_modal_deferral_and_escape_guards() {
    use crate::game::scoring::calculate_score;
    use crate::game::{AwardKind, Trophy};
    use crate::save::LevelRecord;
    use crate::ui::widgets::QuestionId;
    use std::collections::HashMap;

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let mut app = GameApp::new(bundle, Some(120), test_gpu()).expect("GameApp init");

    // Helper: non-platinum records for 0..count
    let make_records = |count: usize, trophy: Trophy| -> HashMap<usize, LevelRecord> {
        (0..count)
            .map(|i| {
                (
                    i,
                    LevelRecord {
                        best_trophy: trophy,
                        best_score: 1000,
                        best_moves: Some(5),
                        best_time_ms: Some(5000),
                    },
                )
            })
            .collect()
    };

    // 1. Level 25 -> 26 unlock chain: must NOT re-fire NextLevel behind LightWater / UnlockMatchNotice.
    app.save.profiles = vec![ProfileData {
        name: "Tester".into(),
        level_index: 24,
        savable_level_index: 24,
        records: make_records(24, Trophy::Gold),
        ..Default::default()
    }];
    app.save.current_profile = Some("Tester".into());
    app.start_campaign_level(24, true);
    let win_gold = calculate_score(4, 5, 5000, 8, 10, 20000, false);
    app.on_campaign_win(win_gold);
    app.trigger_action(UiAction::NextLevel);
    for _ in 0..60 {
        app.update(0.05);
    }
    assert!(app.summary_state.is_none());
    assert_eq!(app.current_level_idx, 25);
    assert!(matches!(
        app.modals.last(),
        Some(ModalState::Award {
            kind: AwardKind::LightWater,
            ..
        })
    ));
    app.on_key_escape();
    assert_eq!(app.modals.len(), 1);
    app.trigger_action(UiAction::DismissAward);
    assert!(matches!(
        app.modals.last(),
        Some(ModalState::Question {
            id: QuestionId::UnlockMatchNotice(_),
            ..
        })
    ));
    for _ in 0..10 {
        app.update(0.05);
    }
    assert_eq!(app.current_level_idx, 25);
    app.on_key_escape();
    assert_eq!(app.modals.len(), 1);
    app.trigger_action(UiAction::QuestionYes);
    assert!(app.modals.is_empty());
    assert_eq!(app.screen, Screen::Playing);
    assert_eq!(app.current_level_idx, 25);

    // 2. Level 100 unlock chain: must NOT launch ChampionScreen early behind UnlockMirrorNotice / Champ.
    app.save.profiles = vec![ProfileData {
        name: "Tester".into(),
        level_index: 99,
        savable_level_index: 99,
        awards: [true, false, false, false],
        records: make_records(99, Trophy::Gold),
        ..Default::default()
    }];
    app.start_campaign_level(99, true);
    app.on_campaign_win(win_gold);
    app.trigger_action(UiAction::NextLevel);
    for _ in 0..60 {
        app.update(0.05);
    }
    assert!(app.summary_state.is_none());
    assert!(app.champion_state.is_none());
    assert_ne!(app.screen, Screen::Champion);
    assert!(matches!(
        app.modals.last(),
        Some(ModalState::Question {
            id: QuestionId::UnlockMirrorNotice(false),
            ..
        })
    ));
    app.on_key_escape();
    assert_eq!(app.modals.len(), 1);
    app.trigger_action(UiAction::QuestionYes);
    assert!(matches!(
        app.modals.last(),
        Some(ModalState::Award {
            kind: AwardKind::Champ,
            ..
        })
    ));
    app.on_key_escape();
    assert_eq!(app.modals.len(), 1);
    app.trigger_action(UiAction::DismissAward);
    assert!(app.modals.is_empty());
    assert_eq!(app.screen, Screen::Champion);
    assert!(app.champion_state.is_some());

    // 3. ULTRALUXER defers SummaryState until dismissed; MIRROR ZEN clears info_banner.
    app.champion_state = None;
    app.save.profiles = vec![ProfileData {
        name: "Tester".into(),
        level_index: 99,
        savable_level_index: 99,
        awards: [true, true, false, false],
        records: make_records(99, Trophy::Platinum),
        ..Default::default()
    }];
    app.start_campaign_level(99, true);
    let win_plat = calculate_score(4, 5, 5000, 4, 5, 10000, true);
    app.on_campaign_win(win_plat);
    assert!(matches!(
        app.modals.last(),
        Some(ModalState::Award {
            kind: AwardKind::Ultralux,
            ..
        })
    ));
    assert!(app.summary_state.is_none());
    for _ in 0..10 {
        app.update(0.05);
    }
    assert!(app.summary_state.is_none());
    app.trigger_action(UiAction::DismissAward);
    assert!(app.modals.is_empty());
    assert_eq!(app.screen, Screen::Summary);
    let deferred_sum = app
        .summary_state
        .as_ref()
        .expect("deferred summary created");
    assert!((deferred_sum.age - 0.0).abs() < 1e-6);

    if let Some(p) = app.save.active_profile_mut() {
        p.mirror_level_index = 49;
    }
    app.execute_launch(LaunchSource::Match { mirror: true });
    app.on_match_win();
    assert!(app.info_banner.is_some());
    app.advance_after_match();
    assert!(app.info_banner.is_none());
    assert!(matches!(
        app.modals.last(),
        Some(ModalState::Award {
            kind: AwardKind::MirrorMaster,
            ..
        })
    ));
    app.modals.clear();

    // 4. Trophy Room retry preserves savable_level_index across win + Next Level and win + Main Menu.
    app.save.profiles = vec![ProfileData {
        name: "Tester".into(),
        level_index: 0,
        savable_level_index: 0,
        records: make_records(100, Trophy::Gold),
        ..Default::default()
    }];
    app.execute_launch(LaunchSource::RetryTrophy {
        idx: 12,
        difficulty: Difficulty::Hard,
    });
    assert!(!app.is_index_savable);
    app.on_campaign_win(win_gold);
    assert_eq!(app.save.active_profile().unwrap().savable_level_index, 0);
    app.trigger_action(UiAction::NextLevel);
    for _ in 0..60 {
        app.update(0.05);
    }
    assert_eq!(app.current_level_idx, 13);
    assert_eq!(app.save.active_profile().unwrap().savable_level_index, 0);
    app.on_campaign_win(win_gold);
    app.trigger_action(UiAction::MainMenuConfirmFromSummary);
    app.trigger_action(UiAction::QuestionYes);
    for _ in 0..60 {
        app.update(0.05);
    }
    assert_eq!(app.save.active_profile().unwrap().savable_level_index, 0);
    assert_eq!(app.save.active_profile().unwrap().level_index, 0);

    // 5. LOTD retry does not stack lotd_score.
    app.execute_launch(LaunchSource::Lotd);
    let lotd_hi = calculate_score(4, 5, 5000, 4, 5, 10000, true);
    let lotd_lo = calculate_score(2, 10, 15000, 4, 5, 10000, false);
    assert!(lotd_hi.total_score > lotd_lo.total_score);
    app.on_campaign_win(lotd_hi);
    assert_eq!(app.lotd_score, lotd_hi.total_score);
    app.trigger_action(UiAction::RetryLevel);
    for _ in 0..60 {
        app.update(0.05);
    }
    app.on_campaign_win(lotd_lo);
    assert_eq!(app.lotd_score, lotd_hi.total_score);

    // 6. Escape does not pause during board_pop_elapsed or tutorial, and pauses freeze transitions.
    app.start_campaign_level(0, false);
    assert!(app.board_pop_elapsed.is_some());
    app.on_key_escape();
    assert_eq!(app.screen, Screen::Playing);
    let pop_before = app.board_pop_elapsed;
    app.trigger_action(UiAction::PauseGame);
    assert_eq!(app.screen, Screen::Paused);
    app.update(0.05);
    assert_eq!(app.board_pop_elapsed, pop_before);
}

#[test]
fn match_mode_rotatable_scramble_and_preview_fade_parity() {
    use crate::game::MatchSim;

    let mut rot_dir_changed = 0usize;
    for seed in 0u64..20 {
        let mut rng = StdRng::seed_from_u64(seed);
        let sim = MatchSim::new(10, true, &mut rng);
        assert!(!sim.is_solved(), "seed {seed} should not start solved");
        for y in 0..sim.level.rows {
            for x in 0..sim.level.cols {
                let play_t = &sim.play_tiles[y][x];
                let goal_t = &sim.goal_tiles[y][x];
                if play_t.can_rotate
                    || matches!(play_t.piece_type, PieceType::Source | PieceType::Drain)
                {
                    assert_eq!(goal_t.piece_type, play_t.piece_type);
                    assert_eq!(goal_t.can_rotate, play_t.can_rotate);
                    assert_eq!(goal_t.shape, play_t.shape);
                }
                if play_t.can_rotate
                    && play_t.shape != PieceShape::Cross
                    && goal_t.direction != play_t.direction
                {
                    rot_dir_changed += 1;
                }
            }
        }
    }
    assert!(
        rot_dir_changed > 0,
        "Match scrambling must rotate can_rotate tiles"
    );

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let gpu = test_gpu();
    let mut app = GameApp::new(bundle, Some(120), gpu.clone()).expect("GameApp init");
    app.start_match_level(10, true, false);

    // Pop-in delay=1s, duration=1s with quad_out easing.
    app.board_pop_elapsed = Some((0.5, 1.5, 4.0, 0));
    assert!((app.match_preview_alpha() - 0.0).abs() < 1e-4);
    app.board_pop_elapsed = Some((1.5, 1.5, 4.0, 0));
    assert!((app.match_preview_alpha() - 0.75).abs() < 1e-4);
    app.board_pop_elapsed = None;
    assert!((app.match_preview_alpha() - 1.0).abs() < 1e-4);

    // Win hide fades out over 1.0s with 1.0 - quad_out(t).
    app.on_match_win();
    assert!((app.match_preview_alpha() - 1.0).abs() < 1e-4);
    if let Some((elapsed, _, _, _)) = app.board_hide.as_mut() {
        *elapsed = 0.5;
    }
    assert!((app.match_preview_alpha() - 0.25).abs() < 1e-4);
    if let Some((elapsed, _, _, _)) = app.board_hide.as_mut() {
        *elapsed = 1.0;
    }
    assert!((app.match_preview_alpha() - 0.0).abs() < 1e-4);

    let sim = app.match_sim.as_ref().unwrap();
    let header_stats = |alpha: f32| -> (u8, bool) {
        let mut c = Canvas::new();
        c.clear((0, 0, 0));
        hud::render_match_hud(
            &mut c,
            &app.renderer.bank,
            &app.renderer.fonts,
            sim,
            0,
            (0.0, 0.0),
            false,
            alpha,
            &hud::HudAnimState::fully_visible(),
            true,
        );
        let px = gpu.render_to_rgba(&c, &app.renderer.bank, 800, 600);
        let mut max_c = 0u8;
        let mut has_dark_blue_stroke = false;
        for y in 40usize..95 {
            for x in 30usize..260 {
                let i = (y * 800 + x) * 4;
                let (r, g, b) = (px[i], px[i + 1], px[i + 2]);
                max_c = max_c.max(r).max(g).max(b);
                if r == 0 && g == 0 && b >= 40 {
                    has_dark_blue_stroke = true;
                }
            }
        }
        (max_c, has_dark_blue_stroke)
    };

    let (max_zero, _) = header_stats(0.0);
    let (max_half, _) = header_stats(0.5);
    let (max_full, has_stroke) = header_stats(1.0);
    assert_eq!(max_zero, 0, "hidden preview must not draw header");
    assert!(
        max_half > 20 && max_half < max_full,
        "half-alpha header ({max_half}) must fade smoothly toward full ({max_full})"
    );
    assert!(
        has_stroke,
        "MIRROR THIS header must use dark blue (0, 0, 64) stroke"
    );
}

#[test]
fn hud_score_single_count_magic_moves_and_stationary_frozen_pipe_parity() {
    use crate::game::SimEvent;
    use crate::render::board::{BoardAnim, BoardRenderer};
    use crate::save::LevelRecord;
    use crate::ui::HideAfterBoard;

    // 1. Gate with 0 Magic tiles still emits TwistPieceSound and keeps moves == 0.
    let gate_no_magic_def = LevelDef {
        index: 0,
        title: "GateTest".into(),
        description: String::new(),
        background_id: "NightHills".into(),
        liquid_color: GameColor::Blue,
        start_delay: 0.0,
        liquid_speed: 30.0,
        rotater_color: GameColor::White,
        mover_color: GameColor::Yellow,
        stationary_color: GameColor::Orange,
        gate_color: GameColor::Violet,
        freeze_color: GameColor::Yellow,
        magic_color: GameColor::Green,
        par_moves: 1,
        par_pipes: 3,
        par_time_ms: 5000,
        cols: 3,
        rows: 1,
        tiles: vec![
            TileDef {
                x: 0,
                y: 0,
                shape: PieceShape::Spigot,
                piece_type: PieceType::Source,
                direction: 3,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
            TileDef {
                x: 1,
                y: 0,
                shape: PieceShape::Straight,
                piece_type: PieceType::Pipe,
                direction: 1,
                is_moveable: false,
                can_rotate: false,
                is_gate: true,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
            TileDef {
                x: 2,
                y: 0,
                shape: PieceShape::Drain,
                piece_type: PieceType::Drain,
                direction: 1,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
        ],
        tutorials: Vec::new(),
    };
    let mut gate_sim = LevelSim::new(gate_no_magic_def, Difficulty::Medium);
    let _ = gate_sim.start_level();
    let mut saw_twist = false;
    for _ in 0..300 {
        for ev in gate_sim.update(1.0 / 60.0) {
            if matches!(ev, SimEvent::PlaySfx("TwistPieceSound")) {
                saw_twist = true;
            }
        }
        if gate_sim.finished {
            break;
        }
    }
    assert!(
        saw_twist,
        "Gate entry must emit TwistPieceSound even with 0 Magic tiles"
    );
    assert_eq!(gate_sim.moves, 0);

    // 2. Synthetic FrozenPipe with is_freeze=false still slows normal flow 8x.
    let make_frozen_type_def = |ptype: PieceType| LevelDef {
        index: 0,
        title: "FrozenTest".into(),
        description: String::new(),
        background_id: "NightHills".into(),
        liquid_color: GameColor::Blue,
        start_delay: 0.0,
        liquid_speed: 24.0,
        rotater_color: GameColor::White,
        mover_color: GameColor::Yellow,
        stationary_color: GameColor::Orange,
        gate_color: GameColor::Violet,
        freeze_color: GameColor::Yellow,
        magic_color: GameColor::Green,
        par_moves: 1,
        par_pipes: 3,
        par_time_ms: 5000,
        cols: 3,
        rows: 1,
        tiles: vec![
            TileDef {
                x: 0,
                y: 0,
                shape: PieceShape::Spigot,
                piece_type: PieceType::Source,
                direction: 3,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
            TileDef {
                x: 1,
                y: 0,
                shape: PieceShape::Straight,
                piece_type: ptype,
                direction: 1,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
            TileDef {
                x: 2,
                y: 0,
                shape: PieceShape::Drain,
                piece_type: PieceType::Drain,
                direction: 1,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
        ],
        tutorials: Vec::new(),
    };
    let mut norm = LevelSim::new(make_frozen_type_def(PieceType::Pipe), Difficulty::Medium);
    let mut froz = LevelSim::new(
        make_frozen_type_def(PieceType::FrozenPipe),
        Difficulty::Medium,
    );
    assert_eq!(froz.tiles[0][1].pipe_color, GameColor::Orange);
    assert!(froz.tiles[0][1].is_frozen());
    let _ = norm.start_level();
    let _ = froz.start_level();
    for _ in 0..300 {
        norm.update(1.0 / 60.0);
        froz.update(1.0 / 60.0);
        if !norm.tiles[0][1].fills.is_empty() && !froz.tiles[0][1].fills.is_empty() {
            break;
        }
    }
    let p0_n = norm.tiles[0][1].fills[0].progress;
    let p0_f = froz.tiles[0][1].fills[0].progress;
    for _ in 0..30 {
        norm.update(1.0 / 60.0);
        froz.update(1.0 / 60.0);
    }
    let dn = norm.tiles[0][1].fills[0].progress - p0_n;
    let df = froz.tiles[0][1].fills[0].progress - p0_f;
    assert!(
        (dn / df - 8.0).abs() < 0.05,
        "expected 8x slowdown on stationary FrozenPipe, got {}",
        dn / df
    );

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");

    // 3. Campaign Level 36: 0 player moves + 2 gate-triggered Magic rotations == 2 moves.
    let mut l36 = LevelSim::new(bundle.campaign_levels[35].clone(), Difficulty::Hard);
    let _ = l36.start_level();
    let _ = l36.skip_tutorials();
    let _ = l36.fast_forward();
    for _ in 0..1200 {
        l36.update(1.0 / 60.0);
        if l36.finished {
            break;
        }
    }
    assert_eq!(l36.won, Some(true));
    assert_eq!(
        l36.moves, 2,
        "Level 36 should count 2 magic rotations toward moves"
    );

    // 4. Campaign Level 26 stationary FrozenPipes are Orange (while moveable ones are Yellow).
    let l26 = LevelSim::new(bundle.campaign_levels[25].clone(), Difficulty::Medium);
    let mut stationary_frozen = 0usize;
    for t in l26.tiles.iter().flatten() {
        if t.piece_type == PieceType::FrozenPipe {
            assert!(t.is_frozen());
            if !t.is_moveable && !t.can_rotate {
                stationary_frozen += 1;
                assert!(!t.is_freeze);
                assert_eq!(t.pipe_color, GameColor::Orange);
            } else if t.is_moveable {
                assert_eq!(t.pipe_color, GameColor::Yellow);
            }
        }
    }
    assert!(
        stationary_frozen > 0,
        "Level 26 should contain stationary FrozenPipe tiles"
    );

    // 5. HUD Score single-count during play, fail rollback, win cap, and Summary HUD hiding.
    let gpu = test_gpu();
    let mut app = GameApp::new(bundle, Some(120), gpu.clone()).expect("GameApp init");
    app.save.current_profile = Some("Tester".into());
    app.save.profiles = vec![ProfileData {
        name: "Tester".into(),
        level_index: 1,
        savable_level_index: 1,
        records: [(
            0,
            LevelRecord {
                best_trophy: Trophy::Gold,
                best_score: 1000,
                best_moves: Some(4),
                best_time_ms: Some(5000),
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    }];
    app.start_campaign_level(1, false);
    app.board_pop_elapsed = None;
    app.info_banner = None;
    if let Some(sim) = app.campaign_sim.as_mut() {
        sim.is_paused = true;
        sim.score = 300;
    }
    for _ in 0..30 {
        app.update(0.05);
    }
    assert_eq!(app.hud_score_roller.value(), 1300);

    // Check rendered HUD counters do not add sim.score a second time.
    let mut c_hud_a = Canvas::new();
    c_hud_a.clear((0, 0, 0));
    let mut sim_clone = app.campaign_sim.as_ref().unwrap().clone();
    let layout = BoardLayout::compute(
        sim_clone.level.cols,
        sim_clone.level.rows,
        hud::campaign_board_area(),
    );
    hud::render_campaign_hud(
        &mut c_hud_a,
        &app.renderer.bank,
        &app.renderer.fonts,
        &sim_clone,
        layout,
        1300,
        sim_clone.moves,
        sim_clone.elapsed_ms as u32,
        0.0,
        None,
        (0.0, 0.0),
        false,
        false,
        &hud::HudAnimState::fully_visible(),
        true,
    );
    let px_a = gpu.render_to_rgba(&c_hud_a, &app.renderer.bank, 800, 600);

    let mut c_hud_b = Canvas::new();
    c_hud_b.clear((0, 0, 0));
    sim_clone.score = 0;
    hud::render_campaign_hud(
        &mut c_hud_b,
        &app.renderer.bank,
        &app.renderer.fonts,
        &sim_clone,
        layout,
        1300,
        sim_clone.moves,
        sim_clone.elapsed_ms as u32,
        0.0,
        None,
        (0.0, 0.0),
        false,
        false,
        &hud::HudAnimState::fully_visible(),
        true,
    );
    let px_b = gpu.render_to_rgba(&c_hud_b, &app.renderer.bank, 800, 600);
    for y in 560usize..598 {
        for x in 690usize..795 {
            let i = (y * 800 + x) * 4;
            assert_eq!(
                &px_a[i..i + 4],
                &px_b[i..i + 4],
                "HUD must display total_score once without re-adding sim.score at ({x},{y})"
            );
        }
    }

    // Fail rolls hud_score_roller back to base score.
    if let Some(sim) = app.campaign_sim.as_mut() {
        sim.finished = true;
        sim.won = Some(false);
    }
    for _ in 0..30 {
        app.update(0.05);
    }
    assert_eq!(app.hud_score_roller.value(), 1000);

    // Win rolls hud_score_roller to 1000 + total_score (not 1000 + 2 * total_score).
    let win_res = calculate_score(4, 10, 5000, 4, 10, 10000, false);
    if let Some(sim) = app.campaign_sim.as_mut() {
        sim.finished = true;
        sim.won = Some(true);
        sim.score = win_res.total_score;
    }
    app.on_campaign_win(win_res);
    app.summary_state = None;
    for _ in 0..30 {
        app.update(0.05);
    }
    assert_eq!(app.hud_score_roller.value(), 1000 + win_res.total_score);

    // Campaign HUD is suppressed on Screen::Summary even while board_hide drops tiles.
    app.cursor_in_window = false;
    app.modals.clear();
    app.blurbs.clear();
    app.leaks.clear();
    app.fireworks.clear();
    app.board_hide = Some((0.0, 0.0, 2.5, HideAfterBoard::None));
    app.screen = Screen::Summary;

    let mut c_bg = Canvas::new();
    app.renderer
        .draw_background(&mut c_bg, &app.assets, &app.bg_anim);
    BoardRenderer::new().draw_board(
        &mut c_bg,
        &app.renderer.bank,
        &app.renderer.fonts,
        &app.campaign_sim.as_ref().unwrap().tiles,
        layout,
        None,
        &[],
        None,
        0.0,
        &[],
        false,
        false,
        BoardAnim::HideDrop {
            elapsed: 0.0,
            delay: 0.0,
        },
        app.board_age,
    );
    let px_bg = gpu.render_to_rgba(&c_bg, &app.renderer.bank, 800, 600);

    let mut c_sum = Canvas::new();
    app.render(&mut c_sum);
    let px_sum = gpu.render_to_rgba(&c_sum, &app.renderer.bank, 800, 600);

    app.screen = Screen::Playing;
    app.hud_anim = hud::HudAnimState::fully_visible();
    let mut c_play = Canvas::new();
    app.render(&mut c_play);
    let px_play = gpu.render_to_rgba(&c_play, &app.renderer.bank, 800, 600);

    let mut play_diff = false;
    for y in 576usize..596 {
        for x in 640usize..790 {
            let i = (y * 800 + x) * 4;
            assert_eq!(
                &px_sum[i..i + 4],
                &px_bg[i..i + 4],
                "Campaign HUD must stay hidden on Screen::Summary at ({x},{y})"
            );
            if px_play[i..i + 4] != px_bg[i..i + 4] {
                play_diff = true;
            }
        }
    }
    assert!(
        play_diff,
        "Screen::Playing should draw campaign HUD counters"
    );
}

#[test]
fn tutorial_html_preservation_and_delayed_pop_timing_parity() {
    use crate::game::SimEvent;

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");

    // 1. Campaign Level 1 tutorials retain <font color> / <i> tags and <br><br> paragraph break.
    let lvl0 = &bundle.campaign_levels[0];
    assert!(
        lvl0.tutorials[0]
            .text
            .contains("<font color=\"#0080FF\">AQUA</font>"),
        "expected preserved AQUA color tag, got: {}",
        lvl0.tutorials[0].text
    );
    assert!(
        lvl0.tutorials[3]
            .text
            .contains("<font color=\"#494949\"><i>DRAG</i></font>"),
        "expected preserved DRAG italic+color tags, got: {}",
        lvl0.tutorials[3].text
    );
    assert!(
        lvl0.tutorials[4].text.contains("\n\n")
            && lvl0.tutorials[4]
                .text
                .contains("<font color=\"#009100\">GREEN.</font>"),
        "expected paragraph break and GREEN color tag, got: {}",
        lvl0.tutorials[4].text
    );

    // 2. Tutorial show/advance schedules PopTwoSound with exact 0.2s delay.
    let mut sim = LevelSim::new(lvl0.clone(), Difficulty::Medium);
    let ev_start = sim.start_level();
    assert!(
        ev_start.iter().any(
            |e| matches!(e, SimEvent::PlaySfxDelayed("PopTwoSound", d) if (*d - 0.2).abs() < 1e-6)
        ),
        "start_level with tutorials should delay PopTwoSound by 0.2s: {:?}",
        ev_start
    );
    let ev_next = sim.next_tutorial();
    assert!(
        ev_next.iter().any(
            |e| matches!(e, SimEvent::PlaySfxDelayed("PopTwoSound", d) if (*d - 0.2).abs() < 1e-6)
        ),
        "next_tutorial should delay PopTwoSound by 0.2s: {:?}",
        ev_next
    );

    // 3. FastForward hint tracks live age, schedules 0.2s PopTwoSound on appear, and resets when hidden.
    let gpu = test_gpu();
    let mut app = GameApp::new(bundle, Some(60), gpu).expect("GameApp init");
    app.launch_level_def(app.assets.campaign_levels[0].clone());
    app.board_pop_elapsed = None;
    app.info_banner = None;
    if let Some(s) = app.campaign_sim.as_mut() {
        s.tutorials_skipped = true;
        s.is_ready = true;
        s.is_started = true;
        s.path_complete = true;
    }
    assert!(app.ff_hint_age.is_none());
    app.audio.clear_delayed();

    app.update(0.05);
    assert_eq!(app.ff_hint_age, Some(0.0));
    assert!(
        app.audio
            .delayed_sfx()
            .iter()
            .any(|(n, d)| *n == "PopTwoSound" && (*d - 0.2).abs() < 1e-6),
        "FastForward hint appearance must schedule PopTwoSound at 0.2s"
    );

    app.update(0.05);
    app.update(0.05);
    assert!((app.ff_hint_age.unwrap() - 0.10).abs() < 1e-5);

    if let Some(s) = app.campaign_sim.as_mut() {
        s.path_complete = false;
    }
    app.update(0.05);
    assert!(app.ff_hint_age.is_none());

    if let Some(s) = app.campaign_sim.as_mut() {
        s.path_complete = true;
    }
    app.update(0.05);
    assert_eq!(app.ff_hint_age, Some(0.0));

    if let Some(s) = app.campaign_sim.as_mut() {
        s.is_accelerated = true;
    }
    app.update(0.05);
    assert!(app.ff_hint_age.is_none());
}

#[test]
fn cursor_wait_scope_hover_gating_and_flow_timer_beeps_parity() {
    use crate::game::SimEvent;
    use crate::render::canvas::DrawCmd;
    use crate::render::tex_bank::GlowPreset;
    use crate::render::{BoardLayout, Canvas, CursorKind};
    use crate::ui::hud;

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");

    // 1. 5s FlowTimer emits 3 AlarmBeepSounds (0:02, 0:01, 0:00), alternates warning_red, and freezes warn_elapsed during fade.
    let timer_def = LevelDef {
        index: 0,
        title: "TimerTest".into(),
        description: String::new(),
        background_id: "NightHills".into(),
        liquid_color: GameColor::Blue,
        start_delay: 5.0,
        liquid_speed: 6.0,
        rotater_color: GameColor::White,
        mover_color: GameColor::Yellow,
        stationary_color: GameColor::Orange,
        gate_color: GameColor::Violet,
        freeze_color: GameColor::Yellow,
        magic_color: GameColor::Green,
        par_moves: 1,
        par_pipes: 3,
        par_time_ms: 5000,
        cols: 3,
        rows: 1,
        tiles: vec![
            TileDef {
                x: 0,
                y: 0,
                shape: PieceShape::Spigot,
                piece_type: PieceType::Source,
                direction: 3,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
            TileDef {
                x: 1,
                y: 0,
                shape: PieceShape::Straight,
                piece_type: PieceType::Pipe,
                direction: 1,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
            TileDef {
                x: 2,
                y: 0,
                shape: PieceShape::Drain,
                piece_type: PieceType::Drain,
                direction: 1,
                is_moveable: false,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
        ],
        tutorials: Vec::new(),
    };
    let mut sim = LevelSim::new(timer_def, Difficulty::Hard);
    let _ = sim.start_level();
    let mut alarm_states = Vec::new();
    for _ in 0..340 {
        for ev in sim.update(1.0 / 60.0) {
            if matches!(ev, SimEvent::PlaySfx("AlarmBeepSound")) {
                let tm = sim.tiles[0][0].timer.unwrap();
                alarm_states.push((tm.last_alarm_sec, tm.warning_red));
            }
        }
        if sim.tiles[0][0].timer.is_some_and(|t| t.fading) {
            break;
        }
    }
    assert_eq!(alarm_states, vec![(2, true), (1, false), (0, true)]);
    let frozen_warn = sim.tiles[0][0].timer.unwrap().warn_elapsed;
    sim.update(0.05);
    assert_eq!(
        sim.tiles[0][0].timer.and_then(|t| t.warn_elapsed),
        frozen_warn,
        "warn_elapsed must freeze once fade starts"
    );

    // 2. WaitCursor only covers swapping tiles and rotatable tiles during swaps; hover glow gated appropriately.
    let gpu = test_gpu();
    let mut app = GameApp::new(bundle, Some(60), gpu).expect("GameApp init");
    let mut board_def = app.assets.campaign_levels[0].clone();
    board_def.cols = 5;
    board_def.rows = 2;
    board_def.tiles = vec![
        TileDef {
            x: 0,
            y: 0,
            shape: PieceShape::Straight,
            piece_type: PieceType::Pipe,
            direction: 0,
            is_moveable: true,
            can_rotate: false,
            is_gate: false,
            is_freeze: false,
            is_magic: false,
            secondary_color: None,
        },
        TileDef {
            x: 1,
            y: 0,
            shape: PieceShape::Straight,
            piece_type: PieceType::Pipe,
            direction: 0,
            is_moveable: true,
            can_rotate: false,
            is_gate: false,
            is_freeze: false,
            is_magic: false,
            secondary_color: None,
        },
        TileDef {
            x: 2,
            y: 0,
            shape: PieceShape::Elbow,
            piece_type: PieceType::Pipe,
            direction: 0,
            is_moveable: true,
            can_rotate: false,
            is_gate: false,
            is_freeze: false,
            is_magic: false,
            secondary_color: None,
        },
        TileDef {
            x: 3,
            y: 0,
            shape: PieceShape::Elbow,
            piece_type: PieceType::Pipe,
            direction: 0,
            is_moveable: false,
            can_rotate: true,
            is_gate: false,
            is_freeze: false,
            is_magic: false,
            secondary_color: None,
        },
        TileDef::dummy_blocker(),
    ];
    board_def.tiles[4].x = 4;
    board_def.tiles[4].y = 0;

    app.launch_level_def(board_def);
    app.board_pop_elapsed = None;
    if let Some(s) = app.campaign_sim.as_mut() {
        s.tutorials_skipped = true;
        s.is_ready = true;
        s.is_started = true;
        s.tiles[0][0].is_swapping = true;
        s.tiles[0][1].is_swapping = true;
        s.swapping_tiles = vec![(0, 0), (1, 0)];
    }

    let layout = {
        let s = app.campaign_sim.as_ref().unwrap();
        BoardLayout::compute(s.level.cols, s.level.rows, hud::campaign_board_area())
    };
    app.mouse_pos = layout.tile_center(0, 0);
    assert_eq!(app.current_cursor(), CursorKind::Wait);
    app.mouse_pos = layout.tile_center(2, 0);
    assert_eq!(app.current_cursor(), CursorKind::Arrow);
    app.mouse_pos = layout.tile_center(3, 0);
    assert_eq!(app.current_cursor(), CursorKind::Wait);
    app.mouse_pos = layout.tile_center(4, 0);
    assert_eq!(app.current_cursor(), CursorKind::Stop);
    app.mouse_pos = (35.0, 570.0);
    assert_eq!(app.current_cursor(), CursorKind::Arrow);
    app.mouse_pos = (5.0, 5.0);
    assert_eq!(app.current_cursor(), CursorKind::Arrow);

    // 3. Hover glow is suppressed during click-to-continue tutorial and during PopIn.
    let hov_tid = app
        .renderer
        .bank
        .glow_named("ElbowClearBitmap", GlowPreset::PipeHover)
        .unwrap()
        .0;
    app.mouse_pos = layout.tile_center(2, 0);
    let mut c_active = Canvas::new();
    app.render(&mut c_active);
    assert!(
        c_active
            .cmds
            .iter()
            .any(|cmd| matches!(cmd, DrawCmd::SpriteTex(tid, _) if *tid == hov_tid)),
        "active non-swapping tile hover should draw PipeHover"
    );

    if let Some(s) = app.campaign_sim.as_mut() {
        s.tutorials_skipped = false;
        s.tutorial_idx = 0; // wait_* all empty -> click-to-continue
    }
    let mut c_ctc = Canvas::new();
    app.render(&mut c_ctc);
    assert!(
        !c_ctc
            .cmds
            .iter()
            .any(|cmd| matches!(cmd, DrawCmd::SpriteTex(tid, _) if *tid == hov_tid)),
        "click-to-continue tutorial must suppress tile hover glow"
    );

    if let Some(s) = app.campaign_sim.as_mut() {
        s.tutorials_skipped = true;
    }
    app.board_pop_elapsed = Some((3.8, 3.5, 4.7, 0));
    let mut c_pop = Canvas::new();
    app.render(&mut c_pop);
    assert!(
        !c_pop
            .cmds
            .iter()
            .any(|cmd| matches!(cmd, DrawCmd::SpriteTex(tid, _) if *tid == hov_tid)),
        "board pop-in must suppress tile hover glow"
    );
    app.board_pop_elapsed = None;

    // 4. Pause fade uses Quad.easeOut over 0.5s (at 0.25s, alpha = 1 - 0.75 = 0.25).
    app.screen = Screen::Paused;
    for _ in 0..5 {
        app.update(0.05);
    }
    assert!(
        (app.pause_fade - 0.25).abs() < 1e-4,
        "got {}",
        app.pause_fade
    );
}

#[test]
fn return_to_title_background_lighten_delay_and_summary_single_lighten_parity() {
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let gpu = test_gpu();
    let mut app = GameApp::new(bundle, Some(60), gpu).expect("GameApp init");

    // 1. Returning to Title from NightHills level preserves NightHills with 2.9s lighten delay (total 3.9s).
    let lvl0 = app.assets.campaign_levels[0].clone();
    assert_eq!(lvl0.background_id, "NightHills");
    app.launch_level_def(lvl0);
    app.bg_anim.age = 4.0;
    app.bg_anim.darken(0.0);
    for _ in 0..40 {
        app.bg_anim.update(0.05);
    }
    assert!(app.bg_anim.darken > 0.5);

    app.return_to_main_menu();
    assert_eq!(app.screen, Screen::Title);
    assert_eq!(app.bg_anim.bg_id, "NightHills");
    assert_eq!(app.bg_anim.darken, 1.0);
    assert!((app.bg_anim.darken_delay - 3.9).abs() < 1e-4);
    assert!(app.bg_anim.outgoing.is_none());

    // 2. Returning to Title from another background (DaySky) starts 1.0s crossfade into fresh NightHills.
    let day_lvl = app
        .assets
        .campaign_levels
        .iter()
        .find(|l| l.background_id == "DaySky")
        .expect("campaign should include DaySky level")
        .clone();
    app.launch_level_def(day_lvl);
    app.bg_anim.outgoing = None;
    app.bg_anim.update(2.0);
    app.return_to_main_menu();
    assert_eq!(app.bg_anim.bg_id, "NightHills");
    assert_eq!(app.bg_anim.age, 0.0);
    assert_eq!(app.bg_anim.darken, 0.0);
    assert_eq!(
        app.bg_anim
            .outgoing
            .as_ref()
            .map(|o| o.state.bg_id.as_str()),
        Some("DaySky")
    );

    // 3. Consecutive NightHills levels: Summary Confirm calls lighten(0.0) once and next-level load preserves it.
    app.bg_anim.outgoing = None;
    app.play_mode = PlayMode::Campaign;
    app.start_campaign_level(0, false);
    if let Some(s) = app.campaign_sim.as_mut() {
        s.is_ready = true;
        s.is_started = true;
    }
    let res = calculate_score(4, 10, 5000, 4, 10, 10000, false);
    app.on_campaign_win(res);
    for _ in 0..80 {
        app.update(0.05);
    }
    assert!(app.bg_anim.darken > 0.9);
    if let Some(sum) = app.summary_state.as_mut() {
        sum.age = 4.0;
    }
    app.trigger_action(UiAction::NextLevel);
    assert!((app.bg_anim.darken_delay - 1.0).abs() < 1e-4);
    assert_eq!(app.bg_anim.target_darken, 0.0);

    for _ in 0..41 {
        app.update(0.05);
    }
    assert_eq!(app.current_level_idx, 1);
    assert_eq!(app.assets.campaign_levels[1].background_id, "NightHills");
    assert_eq!(app.bg_anim.bg_id, "NightHills");
    assert!(
        app.bg_anim.darken > 0.05 && app.bg_anim.darken < 0.7,
        "continuous lighten must not snap to 0 or reset delay, got darken={}",
        app.bg_anim.darken
    );
    assert_eq!(app.bg_anim.darken_delay, 0.0);
}

#[test]
fn splash_overlay_first_run_enter_name_welcome_stagger_and_title_button_gating_parity() {
    use crate::ui::title::{TitleMenuAnim, TitleProfileAnim};

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let gpu = test_gpu();
    let mut app = GameApp::new(bundle, Some(60), gpu).expect("GameApp init");
    app.save.profiles.clear();
    app.save.current_profile = None;

    let step = |app: &mut GameApp, mut rem: f32| {
        while rem > 1e-6 {
            let dt = rem.min(0.05);
            app.update(dt);
            rem -= dt;
        }
    };

    // 1. Splash stays on Screen::Splash until 3.9s, then transitions to Title while keeping splash overlay up to 4.2s.
    step(&mut app, 3.85);
    assert_eq!(app.screen, Screen::Splash);
    assert!(app.splash_overlay_age.is_none());

    step(&mut app, 0.07);
    assert_eq!(app.screen, Screen::Title);
    assert!(app.splash_overlay_age.is_some());
    assert!(app.title_wait_profile_latched);
    assert!(app.modals.is_empty());
    assert!(app.current_buttons().is_empty());

    step(&mut app, 0.35);
    assert!(app.splash_overlay_age.is_none());

    // 2. First-run EnterName modal waits until TitleScreen.onShowComplete at 6.0s; title buttons stay gated while waiting.
    step(&mut app, 5.58);
    assert!(app.title_age > 5.9 && app.title_age < 6.0);
    assert!(app.modals.is_empty());
    assert!(app.current_buttons().is_empty());

    step(&mut app, 0.10);
    assert!(app.title_age >= 6.0);
    assert!(matches!(
        app.modals.last(),
        Some(ModalState::EnterName {
            allow_cancel: false,
            ..
        })
    ));
    assert!(app.current_buttons().len() <= 2);

    // 3. Submitting first profile unlatches buttons and shows "Welcome, ..." + "[Change Profile]" with 0.1s delay.
    if let Some(ModalState::EnterName { input, .. }) = app.modals.last_mut() {
        *input = "Alex".to_string();
    }
    app.trigger_action(UiAction::SubmitName);
    assert!(app.modals.is_empty());
    assert!(!app.title_wait_profile_latched);
    assert_eq!(
        app.title_profile_anim,
        Some(TitleProfileAnim::on_first_profile_created())
    );
    assert!(
        !app.current_buttons()
            .iter()
            .any(|b| b.id == "btn_change_profile"),
        "[Change Profile] must wait 0.6s after creation fade starts"
    );
    step(&mut app, 0.65);
    assert!(
        app.current_buttons()
            .iter()
            .any(|b| b.id == "btn_change_profile")
    );

    // 4. Re-entering Title with an existing profile uses "Welcome back, ..." (5.9s) and "[Change Profile]" (6.1s),
    // and unlocks each button only after its slide/fade tween finishes.
    app.enter_title_screen();
    let expected_return = TitleProfileAnim::on_title_enter();
    assert_eq!(app.title_profile_anim, Some(expected_return));
    assert!(!expected_return.is_new_greeting);

    step(&mut app, 5.45);
    assert!(app.current_buttons().is_empty());

    step(&mut app, 0.07); // 5.52s -> btn_start (4.0 + 1.5 = 5.5) unlocked
    let ids = |a: &GameApp| -> Vec<&'static str> {
        a.current_buttons().into_iter().map(|b| b.id).collect()
    };
    assert_eq!(ids(&app), vec!["btn_start"]);

    step(&mut app, 0.10); // 5.62s -> btn_levelmaker (4.1 + 1.5 = 5.6) unlocked
    assert_eq!(ids(&app), vec!["btn_start", "btn_levelmaker"]);

    step(&mut app, 0.30); // 5.92s -> all 5 main buttons unlocked; profile & mmg still fading
    assert_eq!(
        ids(&app),
        vec![
            "btn_start",
            "btn_levelmaker",
            "btn_trophy",
            "btn_options",
            "btn_quit",
        ]
    );

    step(&mut app, 0.70); // 6.62s -> [Change Profile] (6.1 + 0.5 = 6.6) unlocked, mmg (5.2 + 2.0 = 7.2) still fading
    let ids_662 = ids(&app);
    assert!(ids_662.contains(&"btn_change_profile"));
    assert!(!ids_662.contains(&"btn_mmg"));

    step(&mut app, 0.60); // 7.22s -> mmg unlocked
    assert!(ids(&app).contains(&"btn_mmg"));

    // 5. Submenu transitions fade in over 0.5s, Change Profile returns to Main, and ProfileAccept updates greeting immediately.
    app.trigger_action(UiAction::OpenPlayMenu);
    assert_eq!(app.title_menu_anim, TitleMenuAnim::FadeIn { elapsed: 0.0 });
    step(&mut app, 0.25);
    assert!(
        !app.current_buttons().iter().any(|b| b.id == "btn_lotd"),
        "play submenu buttons must gate clicks during 0.5s FadeIn"
    );
    step(&mut app, 0.30);
    assert!(app.current_buttons().iter().any(|b| b.id == "btn_lotd"));

    app.trigger_action(UiAction::OpenProfileModal);
    assert_eq!(app.title_submenu, TitleSubmenu::Main);
    app.trigger_action(UiAction::ProfileAccept);
    assert_eq!(
        app.title_profile_anim,
        Some(TitleProfileAnim::immediate(true))
    );
}

#[test]
fn summary_unlock_hitbox_and_champion_crossfade_parity() {
    use crate::game::Trophy;
    use crate::render::Canvas;

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let gpu = test_gpu();
    let mut app = GameApp::new(bundle, Some(60), gpu).expect("GameApp init");

    // 1. SummaryScreen buttons stay locked before 3.7s and NoTrophy uses 180x180 hitbox.
    let mut sum = SummaryState::new(calculate_score(4, 5, 6000, 4, 5, 8000, false), 0, 100);
    sum.result.trophy = Trophy::None;
    sum.age = 3.5;
    assert!(build_summary_buttons(&sum).is_empty());
    sum.age = 3.65;
    assert!(build_summary_buttons(&sum).is_empty());
    sum.age = 3.7;
    let sum_btns = build_summary_buttons(&sum);
    let trophy_btn = sum_btns
        .iter()
        .find(|b| b.id == "sum_trophy")
        .expect("NoTrophy must expose retry hitbox at 3.7s");
    assert!(trophy_btn.rect.contains(570.0, 290.0));
    assert!(trophy_btn.rect.contains(730.0, 450.0));

    // 2. Champion -> DirectMainMenu immediately shows Title (with 2.9s lighten & NightHills crossfade) under 1.0s Champion fadeout.
    app.start_campaign_level(99, false);
    assert_eq!(app.bg_anim.bg_id, "OuterSpace");
    app.enter_champion_screen(50_000, true);
    app.champion_state.as_mut().unwrap().age = 9.0;
    app.trigger_action(UiAction::DirectMainMenu);
    assert_eq!(app.screen, Screen::Title);
    assert_eq!(app.bg_anim.bg_id, "NightHills");
    assert_eq!(
        app.bg_anim
            .outgoing
            .as_ref()
            .map(|o| o.state.bg_id.as_str()),
        Some("OuterSpace")
    );
    assert_eq!(app.bg_anim.darken, 0.0);
    assert_eq!(
        app.champion_state.as_ref().unwrap().hiding,
        Some((0.0, false))
    );

    for _ in 0..10 {
        app.update(0.05);
    }
    let ch_mid = app
        .champion_state
        .as_ref()
        .expect("Champion overlay active at 0.5s");
    assert!((ch_mid.hiding.unwrap().0 - 0.5).abs() < 1e-3);
    let mut c = Canvas::new();
    c.begin_frame(1.0);
    app.render(&mut c);
    let star_alpha = c
        .cmds
        .iter()
        .find_map(|cmd| match cmd {
            crate::render::canvas::DrawCmd::Shape(s)
                if (s.meta[0] - 3.0).abs() < 0.1 && s.meta[3] > 1.5 =>
            {
                Some(s.fill[3])
            }
            _ => None,
        })
        .expect("Champion StarField overlay should render on top of Title during crossfade");
    assert!(star_alpha > 0.05 && star_alpha < 0.5);

    for _ in 0..12 {
        app.update(0.05);
    }
    assert!(app.champion_state.is_none());

    // 3. Champion -> DirectQuit fades out on ChampionScreen over 1.0s and sets should_quit on completion.
    app.enter_champion_screen(50_000, true);
    app.champion_state.as_mut().unwrap().age = 9.0;
    app.trigger_action(UiAction::DirectQuit);
    assert_eq!(app.screen, Screen::Champion);
    assert!(!app.should_quit);
    for _ in 0..10 {
        app.update(0.05);
    }
    assert!(!app.should_quit);
    for _ in 0..12 {
        app.update(0.05);
    }
    assert!(app.should_quit);
}

#[test]
fn options_save_on_done_case_sensitive_profile_prune_and_trophy_hover_sfx_parity() {
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let gpu = test_gpu();
    let mut app = GameApp::new(bundle, Some(60), gpu).expect("GameApp init");
    app.save.profiles = vec![ProfileData {
        name: "Alex".into(),
        records: [(
            0,
            LevelRecord {
                best_trophy: Trophy::Gold,
                best_score: 1200,
                best_moves: Some(5),
                best_time_ms: Some(4000),
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    }];
    app.save.current_profile = Some("Alex".into());
    app.screen = Screen::Title;

    // 1. Options widgets update live state without saving or toggling fullscreen until Done.
    let before_saves = app.save.save_count.get();
    app.trigger_action(UiAction::OpenOptions);
    app.trigger_action(UiAction::SetMusicVol(0.25));
    app.trigger_action(UiAction::SetSfxVol(0.8));
    app.trigger_action(UiAction::ToggleColorBlind);
    app.trigger_action(UiAction::ToggleWindowed);
    app.trigger_action(UiAction::CycleFps);
    assert_eq!(
        app.save.save_count.get(),
        before_saves,
        "Options controls must defer save until Done"
    );
    assert!(
        !app.sync_fullscreen_req,
        "Windowed toggle must defer fullscreen application until Done"
    );
    assert!(!app.save.config.fullscreen);
    assert!(app.save.config.color_blind);

    app.trigger_action(UiAction::OptionsDone);
    assert!(app.modals.is_empty());
    assert_eq!(app.save.save_count.get(), before_saves + 1);
    assert!(app.save.config.fullscreen);
    assert!(app.sync_fullscreen_req);

    // 2. Profiles prune list is case-sensitive ("Alex" deleted, "alex" added has empty save).
    let mut save_case = SaveData {
        current_profile: Some("Alex".into()),
        profiles: vec![ProfileData {
            name: "Alex".into(),
            records: [(
                0,
                LevelRecord {
                    best_trophy: Trophy::Gold,
                    best_score: 900,
                    best_moves: None,
                    best_time_ms: None,
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        }],
        ..Default::default()
    };
    save_case.apply_profile_list("alex", &["alex".into()]);
    assert_eq!(save_case.profiles.len(), 1);
    assert_eq!(save_case.profiles[0].name, "alex");
    assert!(
        !save_case.profiles[0].has_saved_game(),
        "Deleting 'Alex' and adding 'alex' must prune case-sensitively"
    );

    // 3. Trophy Room hover plays ClickOneSound on unlocked, adjacent unlocked, locked, and award slots.
    app.screen = Screen::Title;
    app.trigger_action(UiAction::OpenTrophyRoom);
    let _ = app.audio.take_sfx_log();
    // Unlocked slot 0: col 0 row 0 -> center ~(100, 130)
    app.on_mouse_move(100.0, 130.0);
    for _ in 0..7 {
        app.update(0.05);
    }
    assert!(app.trophy_hover_age >= 0.35);
    assert_eq!(app.audio.take_sfx_log(), vec!["ClickOneSound"]);
    // Adjacent unlocked slot 1: col 1 row 0 -> ~(190, 130)
    app.on_mouse_move(190.0, 130.0);
    assert_eq!(app.trophy_hover_age, 0.0);
    assert_eq!(app.audio.take_sfx_log(), vec!["ClickOneSound"]);
    // Locked slot 2: col 2 row 0 -> ~(280, 130)
    app.on_mouse_move(280.0, 130.0);
    assert_eq!(app.audio.take_sfx_log(), vec!["ClickOneSound"]);
    // Award slot on last page: page 3 has campaign levels 78..100 (22 slots) + awards at local idx 22..26 -> row 3, col 1..4
    app.modals.clear();
    app.modals.push(ModalState::TrophyRoom {
        page: 3,
        hover_idx: None,
    });
    let award_y = 90.0 + 3.0 * 90.0 + 30.0;
    let award_x = 57.0 + 1.0 * 90.0 + 30.0;
    app.on_mouse_move(award_x, award_y);
    assert_eq!(app.audio.take_sfx_log(), vec!["ClickOneSound"]);
}

#[test]
fn hud_fades_click_gating_rollback_counters_and_audio_parity() {
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let gpu = test_gpu();
    let mut app = GameApp::new(bundle, Some(60), gpu).expect("GameApp init");
    app.save.profiles = vec![ProfileData {
        name: "Tester".into(),
        ..Default::default()
    }];
    app.save.current_profile = Some("Tester".into());

    let step = |a: &mut GameApp, secs: f32| {
        let mut rem = secs;
        while rem > 1e-6 {
            let dt = rem.min(0.05);
            a.update(dt);
            rem -= dt;
        }
    };

    // 1. HUD overlay fades 0->1 over 1.0s, action buttons stay disabled until 0.8s Strong.easeOut fade after SHOW_COMPLETE.
    app.execute_launch(LaunchSource::NewCampaign(Difficulty::Medium));
    assert!(app.hud_anim.overlay_alpha < 0.01);
    step(&mut app, 0.5);
    assert!(app.hud_anim.overlay_alpha > 0.4 && app.hud_anim.overlay_alpha < 0.99);
    step(&mut app, 0.55);
    assert!((app.hud_anim.overlay_alpha - 1.0).abs() < 1e-4);

    // Finish board pop-in on level 0 (tutorial level -> SkipTutorial slides -70 -> 10 over 0.8s).
    let pop_done = app.board_pop_elapsed.unwrap().2;
    step(&mut app, pop_done - 1.0 + 0.05);
    assert!(app.hud_anim.skip_visible);
    assert!(app.hud_anim.skip_tutorial_y() < 10.0);
    assert!(app.hud_anim.skip_clickable());
    step(&mut app, 0.85);
    assert!((app.hud_anim.skip_tutorial_y() - 10.0).abs() < 0.1);
    app.trigger_action(UiAction::SkipTutorials);
    assert!(!app.hud_tutorial_active);
    step(&mut app, 0.4);
    assert!(
        app.hud_anim.skip_visible
            && app.hud_anim.skip_alpha > 0.01
            && app.hud_anim.skip_alpha < 0.99
    );
    step(&mut app, 0.45);
    assert!(!app.hud_anim.skip_visible);

    // Re-check action button fade & click-gating on a non-tutorial level from Title.
    app.return_to_main_menu();
    let no_tut_idx = app
        .assets
        .campaign_levels
        .iter()
        .position(|l| l.tutorials.is_empty())
        .expect("campaign should include non-tutorial levels");
    app.start_campaign_level(no_tut_idx, false);
    let rem_pop = app.board_pop_elapsed.map(|p| p.2 - p.0).unwrap_or(0.0);
    step(&mut app, rem_pop + 0.1);
    assert!(app.hud_anim.action_visible);
    assert!(
        !app.hud_anim.actions_clickable(),
        "Pause/FastForward must gate clicks immediately at SHOW_COMPLETE while fading in"
    );
    assert!(
        !app.current_buttons()
            .into_iter()
            .find(|b| b.id == "btn_pause")
            .unwrap()
            .enabled
    );
    step(&mut app, 0.35);
    assert!(app.hud_anim.action_alpha > 0.5 && app.hud_anim.action_alpha < 0.999);
    step(&mut app, 0.45);
    assert!(app.hud_anim.actions_clickable());

    // 2. Moves and time roll back to 0 over 1.0s on level restart, while in-level changes snap immediately.
    if let Some(sim) = app.campaign_sim.as_mut() {
        sim.is_paused = true;
        sim.moves = 7;
        sim.elapsed_ms = 9000.0;
    }
    step(&mut app, 0.05);
    assert_eq!(app.hud_moves_roller.value(), 7);
    assert_eq!(app.hud_time_roller.value(), 9000);
    app.launch_level_def(app.assets.campaign_levels[5].clone());
    assert_eq!(app.hud_moves_roller.value(), 7);
    assert_eq!(app.hud_time_roller.value(), 9000);
    step(&mut app, 0.75);
    assert!(
        app.hud_moves_roller.value() > 0 && app.hud_moves_roller.value() < 7,
        "Moves counter should roll back smoothly over 1.0s"
    );
    assert!(
        app.hud_time_roller.value() > 0 && app.hud_time_roller.value() < 9000,
        "Time counter should roll back smoothly over 1.0s"
    );
    step(&mut app, 0.30);
    assert_eq!(app.hud_moves_roller.value(), 0);
    assert_eq!(app.hud_time_roller.value(), 0);

    // 3. Same-background music restarts from 0 instead of early-returning.
    let before_plays = app.audio.music_play_count();
    app.audio.play_music("GlideMusic", true);
    app.audio.play_music("GlideMusic", true);
    assert_eq!(app.audio.music_play_count(), before_plays + 2);

    // 4. Summary hide does not play SwapTwoSound, and Match transition preserves delayed SwapTwoSound.
    let win_res = calculate_score(4, 5, 6000, 4, 5, 8000, false);
    app.on_campaign_win(win_res);
    assert!(app.hud_anim.overlay_alpha > 0.9);
    step(&mut app, 0.5);
    assert!(app.hud_anim.overlay_alpha > 0.05 && app.hud_anim.overlay_alpha < 0.9);
    let _ = app.audio.take_sfx_log();
    app.start_summary_hide(crate::ui::summary::SummaryHideAction::NextLevel);
    assert!(
        !app.audio.take_sfx_log().iter().any(|s| s == "SwapTwoSound"),
        "Summary hide must not play SwapTwoSound"
    );

    app.start_match_level(0, false, true);
    app.audio.clear_delayed();
    app.on_match_win();
    app.advance_after_match();
    assert!(
        app.audio
            .delayed_sfx()
            .iter()
            .any(|(n, _)| *n == "SwapTwoSound"),
        "Match level advance must preserve NICE JOB delayed SwapTwoSound"
    );

    // 5. Extraneous ClickOneSound sites are silent, while valid EnterName SubmitName & OptionsDone play ClickOneSound.
    app.modals.clear();
    let _ = app.audio.take_sfx_log();
    app.trigger_action(UiAction::PauseGame);
    app.trigger_action(UiAction::FastForward);
    app.trigger_action(UiAction::SkipTutorials);
    app.trigger_action(UiAction::OpenMmgNotice);
    app.modals.clear();
    app.trigger_action(UiAction::TrophyPickLevel(0));
    app.modals.clear();
    app.modals.push(ModalState::Profile {
        items: vec!["Tester".into()],
        added: vec![],
        selected: 0,
        hover_slot: None,
    });
    app.trigger_action(UiAction::ProfileNew); // pushes EnterName
    app.trigger_action(UiAction::ProfileDelete);
    app.trigger_action(UiAction::SubmitName); // empty -> fills "Player 1" silently
    app.trigger_action(UiAction::CloseTopModal); // EnterName Cancel -> silent
    assert!(
        !app.audio
            .take_sfx_log()
            .iter()
            .any(|s| s == "ClickOneSound"),
        "Non-AS3 button actions must not play ClickOneSound"
    );
    app.modals.clear();
    app.modals.push(ModalState::EnterName {
        prompt: "Enter your name:".into(),
        input: "Sam".into(),
        allow_cancel: true,
        creating_for_profile_list: false,
        pending_launch_after: false,
    });
    app.trigger_action(UiAction::SubmitName);
    assert_eq!(app.audio.take_sfx_log(), vec!["ClickOneSound"]);

    // 6. Window unfocus mutes music and SFX volumes, and focus regain restores them.
    app.audio.music_volume = 0.6;
    app.audio.sfx_volume = 0.75;
    app.set_window_focused(false);
    assert_eq!(app.audio.effective_music_volume(), 0.0);
    assert_eq!(app.audio.effective_sfx_volume(), 0.0);
    let _ = app.audio.take_sfx_log();
    app.audio.play_sfx("ClickOneSound");
    assert!(app.audio.take_sfx_log().is_empty());
    app.set_window_focused(true);
    assert!((app.audio.effective_music_volume() - 0.6).abs() < 1e-4);
    assert!((app.audio.effective_sfx_volume() - 0.75).abs() < 1e-4);
}

#[test]
fn hdr_detection_platforms_and_options_toggle_gating() {
    use crate::render::HdrOutputMode;
    use crate::render::hdr::{HDR_PAPER_WHITE_DEFAULT, HDR_PEAK_DEFAULT, evaluate_hdr_caps};
    use crate::ui::modals::{
        OPTIONS_SLIDER_X, build_modal_buttons, options_layout, options_layout_for,
        render_modal_stack,
    };
    use wgpu::{DisplayCoarseRange, DisplayHdrInfo, DisplayHeadroom, DisplayLuminance};

    // 1. Cross-platform HDR monitor evaluation & manual calibration rules.
    // Windows HDR active (DXGI + DISPLAYCONFIG_SDR_WHITE_LEVEL = 240 nits, peak = 960 nits)
    let win_hdr = evaluate_hdr_caps(
        Some(HdrOutputMode::ScRgb),
        DisplayHdrInfo {
            luminance: Some(DisplayLuminance {
                max_nits: Some(960.0),
                sdr_white_nits: Some(240.0),
                ..Default::default()
            }),
            coarse: Some(DisplayCoarseRange {
                high_dynamic_range: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(win_hdr.available);
    assert!(!win_hdr.needs_manual_calibration);
    assert!((win_hdr.scrgb_white_scale - 3.0).abs() < 1e-4);
    assert!((win_hdr.peak_headroom - 4.0).abs() < 1e-4);
    // Windows ignores manual slider overrides and follows OS DXGI metrics.
    assert_eq!(
        win_hdr.present_params_with_calibration(400.0, 1400.0),
        win_hdr.present_params()
    );

    // Windows SDR monitor / HDR toggled off (DXGI always reports high_dynamic_range = Some(false))
    let win_sdr = evaluate_hdr_caps(
        Some(HdrOutputMode::ScRgb),
        DisplayHdrInfo {
            luminance: Some(DisplayLuminance {
                max_nits: Some(300.0),
                sdr_white_nits: Some(80.0),
                ..Default::default()
            }),
            coarse: Some(DisplayCoarseRange {
                high_dynamic_range: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(!win_sdr.available);
    assert!(!win_sdr.needs_manual_calibration);
    assert!(win_sdr.present_params().is_none());

    // macOS Metal XDR active
    let mac_hdr = evaluate_hdr_caps(
        Some(HdrOutputMode::ScRgb),
        DisplayHdrInfo {
            headroom: Some(DisplayHeadroom {
                current: Some(3.5),
                potential: Some(8.0),
                reference: None,
            }),
            coarse: Some(DisplayCoarseRange {
                high_dynamic_range: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(mac_hdr.available);
    assert!(!mac_hdr.needs_manual_calibration);
    assert!((mac_hdr.scrgb_white_scale - 1.0).abs() < 1e-4);
    assert!((mac_hdr.peak_headroom - 3.5).abs() < 1e-4);

    // macOS XDR capable panel temporarily at full SDR brightness (potential > 1.0, current = 1.0)
    let mac_xdr_max_bright = evaluate_hdr_caps(
        Some(HdrOutputMode::ScRgb),
        DisplayHdrInfo {
            headroom: Some(DisplayHeadroom {
                current: Some(1.0),
                potential: Some(16.0),
                reference: None,
            }),
            coarse: Some(DisplayCoarseRange {
                high_dynamic_range: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(mac_xdr_max_bright.available);
    assert!(!mac_xdr_max_bright.needs_manual_calibration);

    // macOS SDR monitor (potential <= 1.0)
    let mac_sdr = evaluate_hdr_caps(
        Some(HdrOutputMode::ScRgb),
        DisplayHdrInfo {
            headroom: Some(DisplayHeadroom {
                current: Some(1.0),
                potential: Some(1.0),
                reference: None,
            }),
            coarse: Some(DisplayCoarseRange {
                high_dynamic_range: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(!mac_sdr.available);

    // Linux Wayland / Gamescope HDR uses manual calibration sliders because OS nits are unavailable.
    let wayland_pq = evaluate_hdr_caps(Some(HdrOutputMode::Hdr10Pq), DisplayHdrInfo::default());
    assert!(wayland_pq.available);
    assert!(wayland_pq.needs_manual_calibration);
    assert_eq!(wayland_pq.surface_mode, Some(HdrOutputMode::Hdr10Pq));
    let pq_cal = wayland_pq
        .present_params_with_calibration(300.0, 1200.0)
        .expect("linux calibrated params");
    assert!((pq_cal.sdr_white_nits - 300.0).abs() < 1e-4);
    assert!((pq_cal.peak_headroom - 4.0).abs() < 1e-4);

    let wayland_scrgb = evaluate_hdr_caps(Some(HdrOutputMode::ScRgb), DisplayHdrInfo::default());
    assert!(wayland_scrgb.available);
    assert!(wayland_scrgb.needs_manual_calibration);
    let scrgb_cal = wayland_scrgb
        .present_params_with_calibration(406.0, 1200.0)
        .expect("linux scrgb cal");
    assert!((scrgb_cal.scrgb_white_scale - 2.0).abs() < 1e-3);

    // Linux X11 / SDR surface (no HDR color spaces advertised)
    let x11_sdr = evaluate_hdr_caps(None, DisplayHdrInfo::default());
    assert!(!x11_sdr.available);
    assert!(!x11_sdr.needs_manual_calibration);
    assert!(x11_sdr.present_params().is_none());

    // 2. Options UI disables HDR toggle when unsupported, preserves config, and shows manual sliders on Linux.
    let mut cfg: crate::save::AppConfig = serde_json::from_str(r#"{"fullscreen":true}"#)
        .expect("legacy AppConfig without hdr must parse");
    assert!(!cfg.hdr);
    assert!((cfg.hdr_paper_white_nits - HDR_PAPER_WHITE_DEFAULT).abs() < 1e-4);
    assert!((cfg.hdr_peak_nits - HDR_PEAK_DEFAULT).abs() < 1e-4);
    cfg.hdr = true;
    cfg.hdr_paper_white_nits = 310.0;
    cfg.hdr_peak_nits = 1100.0;
    let json = serde_json::to_string(&cfg).expect("serialize AppConfig");
    let cfg_rt: crate::save::AppConfig = serde_json::from_str(&json).expect("round-trip AppConfig");
    assert!(cfg_rt.hdr);
    assert!((cfg_rt.hdr_paper_white_nits - 310.0).abs() < 1e-4);
    assert!((cfg_rt.hdr_peak_nits - 1100.0).abs() < 1e-4);

    let bundle = match asset_file_if_present() {
        Some(p) => AssetBundle::load_from_path(p).expect("assets"),
        None => crate::tests::empty_bundle(),
    };
    let gpu = test_gpu();
    let mut app = GameApp::new(bundle, Some(120), gpu).expect("GameApp init");
    app.screen = Screen::Title;

    // Headless GPU starts with hdr_available = false.
    assert!(!app.hdr_available);
    assert!(!app.hdr_manual_calibration);
    app.save.config.hdr = true; // e.g. saved from previous HDR session
    app.trigger_action(UiAction::OpenOptions);
    let btns_disabled = build_modal_buttons(
        app.modals.last().unwrap(),
        &app.assets,
        &app.renderer.bank,
        &app.renderer.fonts,
        &app.save,
        app.hdr_available,
        app.hdr_manual_calibration,
    );
    let hdr_btn_off = btns_disabled
        .iter()
        .find(|b| b.id == "opt_hdr")
        .expect("opt_hdr button must exist");
    assert!(
        !hdr_btn_off.enabled,
        "opt_hdr must be disabled when !hdr_available"
    );
    app.trigger_action(UiAction::ToggleHdr);
    app.trigger_action(UiAction::OptionsDone);
    assert!(
        app.save.config.hdr,
        "Opening Options on SDR monitor must preserve previously saved hdr preference without clobbering it"
    );
    assert!(!app.sync_hdr_req);

    // Also test cannot enable from false when !hdr_available:
    app.save.config.hdr = false;
    app.trigger_action(UiAction::OpenOptions);
    app.trigger_action(UiAction::ToggleHdr);
    app.trigger_action(UiAction::OptionsDone);
    assert!(!app.save.config.hdr);

    // Enable HDR + Linux manual calibration -> sliders visible, centered, gated on HDR checked.
    app.set_hdr_display_state(true, true);
    let lay_no_cal = options_layout(false);
    assert!(lay_no_cal.hdr_white_y.is_none() && lay_no_cal.hdr_peak_y.is_none());
    let lay_cal = options_layout_for(false, true);
    assert!(((lay_cal.base_y + lay_cal.bh * 0.5) - 300.0).abs() < 0.5);
    let wy = lay_cal.hdr_white_y.expect("hdr_white_y must be present");
    let py = lay_cal.hdr_peak_y.expect("hdr_peak_y must be present");

    app.trigger_action(UiAction::OpenOptions);
    // Calibration sliders must ignore pointer input while HDR checkbox is unchecked.
    app.on_mouse_down(OPTIONS_SLIDER_X + 60.0, wy + 6.0);
    app.on_mouse_up(OPTIONS_SLIDER_X + 60.0, wy + 6.0);
    assert!((app.save.config.hdr_paper_white_nits - HDR_PAPER_WHITE_DEFAULT).abs() < 1e-4);

    let btns_enabled = build_modal_buttons(
        app.modals.last().unwrap(),
        &app.assets,
        &app.renderer.bank,
        &app.renderer.fonts,
        &app.save,
        app.hdr_available,
        app.hdr_manual_calibration,
    );
    let hdr_btn_on = btns_enabled
        .iter()
        .find(|b| b.id == "opt_hdr")
        .expect("opt_hdr button");
    assert!(hdr_btn_on.enabled);

    let before_saves = app.save.save_count.get();
    let _ = app.audio.take_sfx_log();
    app.trigger_action(UiAction::ToggleHdr);
    assert!(
        app.audio.take_sfx_log().is_empty(),
        "ToggleHdr checkbox must be silent"
    );
    assert!(
        !app.save.config.hdr,
        "ToggleHdr must defer applying until Done"
    );
    assert!(!app.sync_hdr_req);
    assert_eq!(app.save.save_count.get(), before_saves);

    // Now that HDR is pending-checked, calibration sliders respond silently and live.
    let _ = app.audio.take_sfx_log();
    app.on_mouse_down(OPTIONS_SLIDER_X + 60.0, wy + 6.0);
    app.on_mouse_up(OPTIONS_SLIDER_X + 60.0, wy + 6.0);
    app.on_mouse_down(OPTIONS_SLIDER_X + 120.0, py + 6.0);
    app.on_mouse_up(OPTIONS_SLIDER_X + 120.0, py + 6.0);
    assert!(app.audio.take_sfx_log().is_empty());
    assert!((app.save.config.hdr_paper_white_nits - 290.0).abs() < 1e-3);
    assert!((app.save.config.hdr_peak_nits - 1600.0).abs() < 1e-3);
    assert_eq!(app.save.save_count.get(), before_saves);

    // Render check while HDR option + manual calibration sliders are active.
    let mut c_opt = Canvas::new();
    render_modal_stack(
        &mut c_opt,
        &app.assets,
        &app.renderer.bank,
        &app.renderer.fonts,
        &app.save,
        &app.modals,
        &app.modal_ages,
        0.0,
        true,
        true,
        true,
        true,
        0.0,
        (-999.0, -999.0),
    );
    assert!(!c_opt.cmds.is_empty());

    app.trigger_action(UiAction::OptionsDone);
    assert!(app.save.config.hdr);
    assert!(app.sync_hdr_req);
    assert_eq!(app.save.save_count.get(), before_saves + 1);

    // Moving window from HDR monitor back to SDR clears pending HDR if Options is open.
    app.sync_hdr_req = false;
    app.trigger_action(UiAction::OpenOptions);
    app.set_hdr_available(false);
    app.trigger_action(UiAction::ToggleHdr);
    app.trigger_action(UiAction::OptionsDone);
    assert!(!app.sync_hdr_req);
}
