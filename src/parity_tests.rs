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
            t,
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
            1.0,
            (0.0, 0.0),
            false,
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
    assert!(build_champion_buttons(app.champion_state.as_ref().unwrap()).is_empty());
    for _ in 0..10 {
        app.update(0.05);
    }
    assert_eq!(app.screen, Screen::Champion);
    for _ in 0..15 {
        app.update(0.05);
    }
    assert_eq!(app.screen, Screen::Title);
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
