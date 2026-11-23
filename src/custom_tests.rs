// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::AssetBundle;
use crate::custom::{CustomPackStore, LoadedPack, PackSource, sanitize_pack_id, validate_pack_dir};
use crate::game::generator::{GenDifficulty, generate_custom_level};
use crate::game::level_io::{
    CustomPackFile, CustomPackMeta, MAX_CUSTOM_XML_BYTES, default_starter_level, parse_pack_xml,
    write_pack_xml,
};
use crate::game::scoring::calculate_score;
use crate::game::solver::{SolveOutcome, solve_level, validate_level_structure};
use crate::game::types::{GameColor, LevelDef, PieceShape, PieceType, TileDef, TutorialMsg};
use crate::render::{BoardLayout, Canvas};
use crate::save::{ProfileData, SaveData};
use crate::tests::asset_file_if_present;
use crate::tests::test_gpu;
use crate::ui::editor::{EditorTool, PipeBehavior, SolveStatus, editor_board_area};
use crate::ui::title::title_menu_metrics;
use crate::ui::title_data::{TitleSubmenu, menu_entries};
use crate::ui::widgets::{ModalState, QuestionId, UiAction};
use crate::ui::{GameApp, PlayMode, Screen};
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn tile(
    x: usize,
    y: usize,
    shape: PieceShape,
    piece_type: PieceType,
    direction: u8,
    is_moveable: bool,
    can_rotate: bool,
) -> TileDef {
    TileDef {
        x,
        y,
        shape,
        piece_type,
        direction,
        is_moveable,
        can_rotate,
        is_gate: false,
        is_freeze: piece_type == PieceType::FrozenPipe,
        is_magic: false,
        secondary_color: None,
    }
}

#[test]
fn xml_pack_round_trip_synthetic_and_all_campaign_levels() {
    let mut portal_a = tile(1, 0, PieceShape::Spigot, PieceType::Portal, 1, false, false);
    portal_a.secondary_color = Some(GameColor::Violet);
    let mut portal_b = tile(3, 2, PieceShape::Spigot, PieceType::Portal, 3, false, false);
    portal_b.secondary_color = Some(GameColor::Violet);
    let mut gate = tile(1, 1, PieceShape::Elbow, PieceType::Pipe, 0, false, false);
    gate.is_gate = true;
    let mut magic = tile(2, 1, PieceShape::Straight, PieceType::Pipe, 0, false, false);
    magic.is_magic = true;
    let frozen = tile(
        2,
        0,
        PieceShape::Elbow,
        PieceType::FrozenPipe,
        2,
        false,
        true,
    );

    let synthetic = CustomPackFile {
        meta: CustomPackMeta {
            name: "Synth & Pack <One>".to_string(),
            author: "Alex \"Tester\"".to_string(),
            description: "Portals & magic 'test'".to_string(),
        },
        levels: vec![LevelDef {
            index: 0,
            title: "Alpha & Omega <1>".to_string(),
            description: "Test \"quotes\" & 'apostrophes'".to_string(),
            background_id: "SnowyHills".to_string(),
            liquid_color: GameColor::Green,
            start_delay: 7.5,
            liquid_speed: 2.25,
            rotater_color: GameColor::White,
            mover_color: GameColor::Yellow,
            stationary_color: GameColor::Orange,
            gate_color: GameColor::Green,
            freeze_color: GameColor::Blue,
            magic_color: GameColor::Violet,
            par_moves: 12,
            par_pipes: 8,
            par_time_ms: 18500,
            cols: 5,
            rows: 4,
            tiles: vec![
                tile(0, 0, PieceShape::Spigot, PieceType::Source, 3, false, false),
                portal_a,
                frozen,
                gate,
                magic,
                portal_b,
                tile(4, 2, PieceShape::Drain, PieceType::Drain, 1, false, false),
            ],
            tutorials: vec![
                TutorialMsg {
                    tiles: vec![(0, 0), (1, 0)],
                    wait_for_select: Some((1, 1)),
                    wait_for_swap: vec![],
                    wait_for_drag: vec![],
                    hold_hilight: true,
                    text: "Line one\nLine two & more".to_string(),
                },
                TutorialMsg {
                    tiles: vec![],
                    wait_for_select: None,
                    wait_for_swap: vec![(1, 0), (2, 0)],
                    wait_for_drag: vec![(2, 1), (3, 1)],
                    hold_hilight: true,
                    text: "Second message".to_string(),
                },
            ],
        }],
    };

    let xml = write_pack_xml(&synthetic);
    let parsed = parse_pack_xml(&xml, "fallback").expect("synthetic pack should parse");
    assert_eq!(parsed, synthetic);

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let camp_pack = CustomPackFile {
        meta: CustomPackMeta {
            name: "Campaign Export".to_string(),
            author: String::new(),
            description: String::new(),
        },
        levels: bundle.campaign_levels.clone(),
    };
    let camp_xml = write_pack_xml(&camp_pack);
    let reparsed = parse_pack_xml(&camp_xml, "fallback").expect("campaign pack should parse");
    assert_eq!(reparsed.levels.len(), bundle.campaign_levels.len());
    for (orig, rt) in bundle.campaign_levels.iter().zip(reparsed.levels.iter()) {
        assert_eq!(orig, rt, "level {} differed on round-trip", orig.index);
    }
}

#[test]
fn custom_pack_store_lifecycle_and_safety() {
    assert_eq!(sanitize_pack_id("../Evil/Pack! "), "evil-pack");
    assert_eq!(sanitize_pack_id("CON"), "pack-con");
    assert_eq!(sanitize_pack_id("   "), "pack");
    assert_eq!(sanitize_pack_id(&"a".repeat(80)).len(), 48);

    let root = std::env::temp_dir().join(format!("aqualux_custom_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let primary = root.join("primary");
    let portable = root.join("portable");
    let drop_dir = root.join("drop");
    let progress_path = root.join("custom_progress.json");
    fs::create_dir_all(&portable).unwrap();
    fs::create_dir_all(&drop_dir).unwrap();

    // Portable pack
    let port_pack = CustomPackFile {
        meta: CustomPackMeta {
            name: "Portable Pack".to_string(),
            ..Default::default()
        },
        levels: vec![default_starter_level(0)],
    };
    fs::write(portable.join("shared.xml"), write_pack_xml(&port_pack)).unwrap();

    let mut store = CustomPackStore::load_with_dirs(
        primary.clone(),
        vec![(portable.clone(), PackSource::PortableDir)],
        progress_path.clone(),
    );
    assert_eq!(store.packs.len(), 1);
    assert!(!store.packs[0].writable);
    assert!(store.delete_pack("shared").is_err());

    // Saving same id in primary overrides portable pack
    let override_pack = CustomPackFile {
        meta: CustomPackMeta {
            name: "Primary Override".to_string(),
            ..Default::default()
        },
        levels: vec![default_starter_level(0), default_starter_level(1)],
    };
    store.save_pack("shared", override_pack).unwrap();
    assert_eq!(store.packs.len(), 1);
    assert!(store.packs[0].writable);
    assert_eq!(store.packs[0].pack.meta.name, "Primary Override");

    // Import + collision rename
    let src_file = drop_dir.join("MyLevel.xml");
    fs::write(
        &src_file,
        write_pack_xml(&CustomPackFile {
            meta: CustomPackMeta {
                name: "Dropped".to_string(),
                ..Default::default()
            },
            levels: vec![default_starter_level(0)],
        }),
    )
    .unwrap();
    let id1 = store.import_xml_file(&src_file).unwrap();
    let id2 = store.import_xml_file(&src_file).unwrap();
    assert_eq!(id1, "mylevel");
    assert_eq!(id2, "mylevel-2");

    // Reject oversized XML
    let big_file = drop_dir.join("huge.xml");
    fs::write(&big_file, "x".repeat(MAX_CUSTOM_XML_BYTES + 10)).unwrap();
    assert!(store.import_xml_file(&big_file).is_err());

    // Progress isolation + disk save
    let res = calculate_score(4, 5, 6000, 6, 5, 15000, true);
    store.record_win("mylevel", 0, res, 4, 6000);
    store.save_progress_to_disk().unwrap();
    let reloaded = CustomPackStore::load_with_dirs(
        primary.clone(),
        vec![(portable, PackSource::PortableDir)],
        progress_path,
    );
    assert_eq!(
        reloaded.progress.packs["mylevel"][&0].best_trophy,
        res.trophy
    );

    store.delete_pack("mylevel-2").unwrap();
    assert!(!primary.join("mylevel-2.xml").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn solver_validation_and_mechanics() {
    // Structural checks
    let mut bad = default_starter_level(0);
    bad.tiles.retain(|t| t.piece_type != PieceType::Source);
    assert!(!validate_level_structure(&bad).is_empty());

    let mut bad_drain = default_starter_level(0);
    bad_drain.tiles.retain(|t| t.piece_type != PieceType::Drain);
    assert!(!validate_level_structure(&bad_drain).is_empty());

    let mut two_sources = default_starter_level(0);
    two_sources.tiles[0] = tile(0, 0, PieceShape::Spigot, PieceType::Source, 3, false, false);
    assert!(!validate_level_structure(&two_sources).is_empty());

    let mut two_drains = default_starter_level(0);
    two_drains.tiles[1] = tile(1, 0, PieceShape::Drain, PieceType::Drain, 1, false, false);
    assert!(!validate_level_structure(&two_drains).is_empty());

    let mut oob_src = default_starter_level(0);
    oob_src
        .tiles
        .iter_mut()
        .find(|t| t.piece_type == PieceType::Source)
        .unwrap()
        .direction = 1; // Left from x=0 is out of bounds
    assert!(!validate_level_structure(&oob_src).is_empty());

    let mut unpaired = default_starter_level(0);
    let mut p = tile(0, 0, PieceShape::Spigot, PieceType::Portal, 3, false, false);
    p.secondary_color = Some(GameColor::Red);
    unpaired.tiles[0] = p;
    assert!(!validate_level_structure(&unpaired).is_empty());

    // Unsolvable blocker wall
    let unsolvable = LevelDef {
        cols: 3,
        rows: 2,
        tiles: vec![
            tile(0, 0, PieceShape::Spigot, PieceType::Source, 3, false, false),
            tile(1, 0, PieceShape::Block, PieceType::Blocker, 0, false, false),
            tile(2, 0, PieceShape::Drain, PieceType::Drain, 1, false, false),
            tile(0, 1, PieceShape::Block, PieceType::Blocker, 0, false, false),
            tile(1, 1, PieceShape::Block, PieceType::Blocker, 0, false, false),
            tile(2, 1, PieceShape::Block, PieceType::Blocker, 0, false, false),
        ],
        ..default_starter_level(0)
    };
    assert!(matches!(
        solve_level(&unsolvable, Duration::from_millis(100)),
        SolveOutcome::Unsolvable(_)
    ));

    // (a) Movable swap puzzle: Straight placed at (1,1), need at (1,0)
    let movable_lvl = LevelDef {
        cols: 3,
        rows: 2,
        tiles: vec![
            tile(0, 0, PieceShape::Spigot, PieceType::Source, 3, false, false),
            tile(1, 0, PieceShape::Elbow, PieceType::Pipe, 0, true, false),
            tile(2, 0, PieceShape::Drain, PieceType::Drain, 1, false, false),
            tile(0, 1, PieceShape::Block, PieceType::Blocker, 0, false, false),
            tile(1, 1, PieceShape::Straight, PieceType::Pipe, 1, true, false),
            tile(2, 1, PieceShape::Block, PieceType::Blocker, 0, false, false),
        ],
        ..default_starter_level(0)
    };
    match solve_level(&movable_lvl, Duration::from_millis(200)) {
        SolveOutcome::Solvable(res) => assert!(res.verified_by_sim),
        other => panic!("expected movable puzzle solvable, got {:?}", other),
    }

    // (b) Portal teleport
    let mut pa = tile(1, 0, PieceShape::Spigot, PieceType::Portal, 1, false, false);
    pa.secondary_color = Some(GameColor::Violet);
    let mut pb = tile(1, 1, PieceShape::Spigot, PieceType::Portal, 3, false, false);
    pb.secondary_color = Some(GameColor::Violet);
    let portal_lvl = LevelDef {
        cols: 3,
        rows: 2,
        tiles: vec![
            tile(0, 0, PieceShape::Spigot, PieceType::Source, 3, false, false),
            pa,
            tile(2, 0, PieceShape::Block, PieceType::Blocker, 0, false, false),
            tile(0, 1, PieceShape::Block, PieceType::Blocker, 0, false, false),
            pb,
            tile(2, 1, PieceShape::Drain, PieceType::Drain, 1, false, false),
        ],
        ..default_starter_level(0)
    };
    match solve_level(&portal_lvl, Duration::from_millis(200)) {
        SolveOutcome::Solvable(res) => assert!(res.verified_by_sim),
        other => panic!("expected portal puzzle solvable, got {:?}", other),
    }

    // (c) Cross double-traversal
    let cross_lvl = LevelDef {
        cols: 4,
        rows: 3,
        tiles: vec![
            tile(0, 1, PieceShape::Spigot, PieceType::Source, 3, false, false),
            tile(1, 1, PieceShape::Cross, PieceType::Pipe, 0, false, false),
            tile(2, 1, PieceShape::Elbow, PieceType::Pipe, 3, false, false),
            tile(2, 0, PieceShape::Elbow, PieceType::Pipe, 2, false, false),
            tile(1, 0, PieceShape::Elbow, PieceType::Pipe, 1, false, true),
            tile(1, 2, PieceShape::Drain, PieceType::Drain, 2, false, false),
        ],
        ..default_starter_level(0)
    };
    match solve_level(&cross_lvl, Duration::from_millis(200)) {
        SolveOutcome::Solvable(res) => assert!(res.verified_by_sim),
        other => panic!("expected cross double-traversal solvable, got {:?}", other),
    }

    // (d) Gate + Magic mod 4: Gate at (1,0) rotates Magic Straight at (2,0) from vertical (0) -> horizontal (1)
    let mut g = tile(1, 0, PieceShape::Straight, PieceType::Pipe, 1, false, false);
    g.is_gate = true;
    let mut m = tile(2, 0, PieceShape::Straight, PieceType::Pipe, 0, false, false);
    m.is_magic = true;
    let gate_lvl = LevelDef {
        cols: 4,
        rows: 2,
        tiles: vec![
            tile(0, 0, PieceShape::Spigot, PieceType::Source, 3, false, false),
            g,
            m,
            tile(3, 0, PieceShape::Drain, PieceType::Drain, 1, false, false),
        ],
        ..default_starter_level(0)
    };
    match solve_level(&gate_lvl, Duration::from_millis(200)) {
        SolveOutcome::Solvable(res) => assert!(res.verified_by_sim),
        other => panic!("expected gate+magic puzzle solvable, got {:?}", other),
    }

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let mut solved_count = 0usize;
    for lvl in &bundle.campaign_levels {
        if let SolveOutcome::Solvable(res) = solve_level(lvl, Duration::from_millis(250)) {
            assert!(
                res.verified_by_sim,
                "campaign level {} solved but failed LevelSim verification",
                lvl.index + 1
            );
            solved_count += 1;
        }
    }
    assert!(
        solved_count >= 95,
        "expected >= 95/100 campaign levels solved, got {}/{}",
        solved_count,
        bundle.campaign_levels.len()
    );
}

#[test]
fn procedural_generator_produces_verified_solvable_levels() {
    for (diff_idx, diff) in [
        GenDifficulty::Easy,
        GenDifficulty::Medium,
        GenDifficulty::Hard,
    ]
    .into_iter()
    .enumerate()
    {
        for seed in 0..3u64 {
            let mut rng = StdRng::seed_from_u64(1000 + diff_idx as u64 * 10 + seed);
            let lvl = generate_custom_level(diff, &mut rng);
            let issues = validate_level_structure(&lvl);
            assert!(
                issues.is_empty(),
                "generated {:?} level had structural issues: {:?}",
                diff,
                issues
            );
            match solve_level(&lvl, Duration::from_millis(250)) {
                SolveOutcome::Solvable(res) => assert!(res.verified_by_sim),
                other => panic!(
                    "generated {:?} level (seed {}) not solved: {:?}",
                    diff, seed, other
                ),
            }
        }
    }
}

#[test]
fn title_play_choices_includes_custom_packs_and_layout_fits() {
    let mut save = SaveData::default();
    let main_entries = menu_entries(TitleSubmenu::Main, &save);
    assert!(main_entries.iter().any(|e| {
        e.id == "btn_levelmaker"
            && e.action
                == Some(UiAction::OpenCustomHub {
                    play_focused: false,
                })
    }));

    let unsaved_play = menu_entries(TitleSubmenu::PlayChoices, &save);
    assert!(unsaved_play.iter().any(|e| {
        e.id == "btn_custom_packs"
            && e.action == Some(UiAction::OpenCustomHub { play_focused: true })
    }));

    save.current_profile = Some("Max".to_string());
    save.profiles.push(ProfileData {
        name: "Max".to_string(),
        awards: [true, true, false, false],
        records: [(0, Default::default())].into_iter().collect(),
        ..Default::default()
    });
    let full_play = menu_entries(TitleSubmenu::PlayChoices, &save);
    assert!(full_play.iter().any(|e| e.id == "btn_custom_packs"));
    let (base_y, spacing, _) = title_menu_metrics(full_play.len());
    let bottom_of_back = base_y + full_play.len() as f32 * spacing;
    assert!(bottom_of_back < 540.0);
    let info_y = (bottom_of_back + 15.0).min(484.0);
    assert!(info_y + 4.0 * 21.0 < 572.0);
}

#[test]
fn custom_hub_pack_and_level_management_and_drop_import() {
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let root = std::env::temp_dir().join(format!("aqualux_hub_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let primary = root.join("custom_packs");
    let drop_dir = root.join("drop");
    fs::create_dir_all(&drop_dir).unwrap();

    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let mut app =
        GameApp::new_with_custom_dir(bundle, Some(120), Some(&primary), test_gpu()).expect("init");
    app.open_custom_hub(false);
    assert_eq!(app.screen, Screen::CustomHub);
    assert_eq!(app.custom_packs.packs.len(), 1);
    let mut canvas = Canvas::new();
    canvas.begin_frame(1.0);
    app.render(&mut canvas);

    // Create pack via TextPrompt
    app.trigger_action(UiAction::HubNewPack);
    for ch in "Second Pack".chars() {
        app.on_text_char(ch);
    }
    app.on_key_enter();
    assert_eq!(app.custom_packs.packs.len(), 2);

    // Rename pack
    app.trigger_action(UiAction::HubRenamePack);
    while app.has_text_modal() && app.modals.last().is_some() {
        if matches!(
            app.modals.last(),
            Some(crate::ui::widgets::ModalState::TextPrompt { input, .. }) if input.is_empty()
        ) {
            break;
        }
        app.on_key_backspace();
    }
    for ch in "Renamed Pack".chars() {
        app.on_text_char(ch);
    }
    app.on_key_enter();
    assert!(
        app.custom_packs
            .packs
            .iter()
            .any(|p| p.pack.meta.name == "Renamed Pack")
    );

    // Add, generate, duplicate, reorder, and delete level
    app.select_pack_by_id("second-pack");
    app.trigger_action(UiAction::HubNewLevel);
    app.trigger_action(UiAction::HubGenerateLevel);
    assert_eq!(
        app.custom_packs.packs[app.custom_hub.selected_pack]
            .pack
            .levels
            .len(),
        3
    );
    app.trigger_action(UiAction::HubDuplicateLevel);
    assert_eq!(
        app.custom_packs.packs[app.custom_hub.selected_pack]
            .pack
            .levels
            .len(),
        4
    );
    let moved_title = app.custom_packs.packs[app.custom_hub.selected_pack]
        .pack
        .levels[app.custom_hub.selected_level]
        .title
        .clone();
    app.trigger_action(UiAction::HubMoveLevelUp);
    assert_eq!(
        app.custom_packs.packs[app.custom_hub.selected_pack]
            .pack
            .levels[app.custom_hub.selected_level]
            .title,
        moved_title
    );
    app.trigger_action(UiAction::HubDeleteLevel);
    app.trigger_action(UiAction::QuestionYes);
    assert_eq!(
        app.custom_packs.packs[app.custom_hub.selected_pack]
            .pack
            .levels
            .len(),
        3
    );

    // Drop .xml import vs rejected .txt drop
    let xml_src = drop_dir.join("DroppedPack.xml");
    fs::write(
        &xml_src,
        write_pack_xml(&CustomPackFile {
            meta: CustomPackMeta {
                name: "Dropped Pack".to_string(),
                ..Default::default()
            },
            levels: vec![default_starter_level(0)],
        }),
    )
    .unwrap();
    app.on_dropped_file(&xml_src);
    assert!(
        app.custom_packs
            .packs
            .iter()
            .any(|p| p.pack.meta.name == "Dropped Pack")
    );

    let txt_src = drop_dir.join("notes.txt");
    fs::write(&txt_src, "ignore").unwrap();
    let count_before = app.custom_packs.packs.len();
    app.on_dropped_file(&txt_src);
    assert_eq!(app.custom_packs.packs.len(), count_before);

    // Delete pack
    app.select_pack_by_id("droppedpack");
    app.trigger_action(UiAction::HubDeletePack);
    app.trigger_action(UiAction::QuestionYes);
    assert!(!app.custom_packs.packs.iter().any(|p| p.id == "droppedpack"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn custom_pack_playback_and_isolated_progress() {
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let root = std::env::temp_dir().join(format!("aqualux_play_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let primary = root.join("custom_packs");

    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let mut app =
        GameApp::new_with_custom_dir(bundle, Some(120), Some(&primary), test_gpu()).expect("init");
    let camp_records_before = app
        .save
        .active_profile()
        .map(|p| (p.records.len(), p.total_score()));

    app.custom_packs
        .save_pack(
            "two-pack",
            CustomPackFile {
                meta: CustomPackMeta {
                    name: "Two Level Pack".to_string(),
                    ..Default::default()
                },
                levels: vec![default_starter_level(0), default_starter_level(1)],
            },
        )
        .unwrap();

    app.launch_custom_pack_level("two-pack", 0, crate::game::Difficulty::Hard, 0);
    assert_eq!(app.screen, Screen::Playing);
    assert_ne!(app.play_mode, PlayMode::Campaign);
    assert!(matches!(
        app.play_mode,
        PlayMode::CustomPack { level_idx: 0, .. }
    ));

    let res = calculate_score(4, 5, 5000, 6, 5, 15000, true);
    app.on_campaign_win(res);
    assert_eq!(app.screen, Screen::Summary);
    assert_eq!(
        app.custom_packs.progress.packs["two-pack"][&0].best_trophy,
        res.trophy
    );
    assert_eq!(
        app.save
            .active_profile()
            .map(|p| (p.records.len(), p.total_score())),
        camp_records_before
    );

    app.advance_after_summary();
    assert!(matches!(
        app.play_mode,
        PlayMode::CustomPack { level_idx: 1, .. }
    ));

    app.on_campaign_win(res);
    app.advance_after_summary();
    assert_eq!(app.screen, Screen::CustomHub);
    assert_eq!(app.play_mode, PlayMode::Campaign);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn editor_grid_editing_undo_redo_resize_verify_autopar_and_save() {
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let root = std::env::temp_dir().join(format!("aqualux_editor_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let primary = root.join("custom_packs");

    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let mut app =
        GameApp::new_with_custom_dir(bundle, Some(120), Some(&primary), test_gpu()).expect("init");
    app.open_custom_hub(false);
    app.trigger_action(UiAction::HubEditLevel);
    assert_eq!(app.screen, Screen::Editor);
    assert!(app.editor.is_some());
    {
        let draft = &app.editor.as_ref().unwrap().draft;
        assert_eq!(draft.rotater_color, GameColor::White);
        assert_eq!(draft.mover_color, GameColor::Yellow);
        assert_eq!(draft.stationary_color, GameColor::Orange);
        assert_eq!(draft.gate_color, GameColor::Violet);
        assert_eq!(draft.freeze_color, GameColor::Yellow);
        assert_eq!(draft.magic_color, GameColor::Green);
    }

    let mut canvas = Canvas::new();
    canvas.begin_frame(1.0);
    app.render(&mut canvas);

    // Paint an Elbow, test single-Source relocation invariant, right-click rotate, shortcuts, undo/redo
    app.trigger_action(UiAction::EditorSelectTool(EditorTool::Elbow));
    assert!(app.editor.as_mut().unwrap().paint_cell(1, 1, true));
    let cell_1_1_dir = app.editor.as_ref().unwrap().draft.tiles
        [app.editor.as_ref().unwrap().tile_index(1, 1).unwrap()]
    .direction;

    let layout = BoardLayout::compute(5, 5, editor_board_area());
    let (cx, cy) = layout.tile_center(1, 1);
    app.on_right_click(cx, cy);
    let cell_1_1_dir_after = app.editor.as_ref().unwrap().draft.tiles
        [app.editor.as_ref().unwrap().tile_index(1, 1).unwrap()]
    .direction;
    assert_eq!(cell_1_1_dir_after, (cell_1_1_dir + 1) % 4);

    // Relocate Source to (0, 1); previous Source at (0, 2) must turn into Blocker
    app.trigger_action(UiAction::EditorSelectTool(EditorTool::Source));
    assert!(app.editor.as_mut().unwrap().paint_cell(0, 1, true));
    let source_count = app
        .editor
        .as_ref()
        .unwrap()
        .draft
        .tiles
        .iter()
        .filter(|t| t.piece_type == PieceType::Source)
        .count();
    assert_eq!(source_count, 1);

    // Undo Source move and test keyboard shortcuts ('r', '1', Ctrl+Z, Ctrl+Y)
    app.on_shortcut('z', true, false);
    assert_eq!(
        app.editor
            .as_ref()
            .unwrap()
            .draft
            .tiles
            .iter()
            .find(|t| t.piece_type == PieceType::Source)
            .map(|t| (t.x, t.y)),
        Some((0, 2))
    );
    app.on_shortcut('y', true, false);
    assert_eq!(
        app.editor
            .as_ref()
            .unwrap()
            .draft
            .tiles
            .iter()
            .find(|t| t.piece_type == PieceType::Source)
            .map(|t| (t.x, t.y)),
        Some((0, 1))
    );
    // Revert back to solvable starter source at (0, 2)
    app.on_shortcut('z', true, false);

    // Relocate Drain to (4, 1); previous Drain at (4, 2) must turn into Blocker
    app.trigger_action(UiAction::EditorSelectTool(EditorTool::Drain));
    assert!(app.editor.as_mut().unwrap().paint_cell(4, 1, true));
    let drain_count = app
        .editor
        .as_ref()
        .unwrap()
        .draft
        .tiles
        .iter()
        .filter(|t| t.piece_type == PieceType::Drain)
        .count();
    assert_eq!(drain_count, 1);
    app.on_shortcut('z', true, false);
    assert_eq!(
        app.editor
            .as_ref()
            .unwrap()
            .draft
            .tiles
            .iter()
            .find(|t| t.piece_type == PieceType::Drain)
            .map(|t| (t.x, t.y)),
        Some((4, 2))
    );

    // Gate/Magic force Fixed behavior; choosing Mover clears Gate/Magic
    app.trigger_action(UiAction::EditorSelectTool(EditorTool::Elbow));
    app.trigger_action(UiAction::EditorToggleGate);
    assert_eq!(app.editor.as_ref().unwrap().behavior, PipeBehavior::Fixed);
    assert!(app.editor.as_ref().unwrap().mod_gate);
    app.trigger_action(UiAction::EditorToggleMagic);
    assert_eq!(app.editor.as_ref().unwrap().behavior, PipeBehavior::Fixed);
    assert!(!app.editor.as_ref().unwrap().mod_gate);
    assert!(app.editor.as_ref().unwrap().mod_magic);
    app.trigger_action(UiAction::EditorSetBehavior(PipeBehavior::Mover));
    assert!(!app.editor.as_ref().unwrap().mod_gate);
    assert!(!app.editor.as_ref().unwrap().mod_magic);

    let dir_before = app.editor.as_ref().unwrap().brush_dir;
    app.on_shortcut('r', false, false);
    assert_eq!(app.editor.as_ref().unwrap().brush_dir, (dir_before + 1) % 4);
    app.on_shortcut('2', false, false);
    assert_eq!(app.editor.as_ref().unwrap().tool, EditorTool::Straight);

    // Resize grid & undo restoration
    app.trigger_action(UiAction::EditorResizeCols(1));
    assert_eq!(app.editor.as_ref().unwrap().draft.cols, 6);
    app.trigger_action(UiAction::EditorResizeRows(-1));
    assert_eq!(app.editor.as_ref().unwrap().draft.rows, 4);
    app.trigger_action(UiAction::EditorUndo);
    app.trigger_action(UiAction::EditorUndo);
    assert_eq!(app.editor.as_ref().unwrap().draft.cols, 5);
    assert_eq!(app.editor.as_ref().unwrap().draft.rows, 5);

    // Verify solvable & render solution overlay
    app.trigger_action(UiAction::EditorVerify);
    let suggested = match &app.editor.as_ref().unwrap().solve_status {
        SolveStatus::Solvable(res) => {
            assert!(res.verified_by_sim);
            (
                res.suggested_par_moves,
                res.suggested_par_pipes,
                res.suggested_par_time_ms,
            )
        }
        other => panic!("expected Solvable in editor, got {:?}", other),
    };
    assert!(app.editor.as_ref().unwrap().show_solution_path);
    canvas.begin_frame(1.0);
    app.render(&mut canvas);

    // Auto-Par updates par values and marks dirty
    app.trigger_action(UiAction::EditorAutoPar);
    {
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.draft.par_moves, suggested.0);
        assert_eq!(ed.draft.par_pipes, suggested.1);
        assert_eq!(ed.draft.par_time_ms, suggested.2);
        assert!(ed.dirty);
    }

    // LevelProps modal: render, edit title, cycle background & liquid, apply
    app.trigger_action(UiAction::EditorOpenProps);
    assert!(matches!(app.modals.last(), Some(ModalState::LevelProps(_))));
    canvas.begin_frame(1.0);
    app.render(&mut canvas);

    app.trigger_action(UiAction::PropsEditTitle);
    while matches!(
        app.modals.last(),
        Some(ModalState::TextPrompt { input, .. }) if !input.is_empty()
    ) {
        app.on_key_backspace();
    }
    for ch in "Edited Stage".chars() {
        app.on_text_char(ch);
    }
    app.on_key_enter();
    app.trigger_action(UiAction::PropsCycleBg);
    app.trigger_action(UiAction::PropsCycleLiquid);
    app.trigger_action(UiAction::PropsAdjustSpeed(1));
    app.trigger_action(UiAction::PropsAdjustDelay(-1));
    app.trigger_action(UiAction::PropsApply);
    assert!(app.modals.is_empty());
    assert_eq!(app.editor.as_ref().unwrap().draft.title, "Edited Stage");
    assert_eq!(app.editor.as_ref().unwrap().draft.background_id, "DaySky");

    // Save to disk & verify reload persistence
    app.trigger_action(UiAction::EditorSave);
    assert!(!app.editor.as_ref().unwrap().dirty);

    let reloaded = CustomPackStore::load_with_dirs(primary, Vec::new(), root.join("prog.json"));
    let saved_lvl = &reloaded.packs[0].pack.levels[0];
    assert_eq!(saved_lvl.title, "Edited Stage");
    assert_eq!(saved_lvl.background_id, "DaySky");
    assert_eq!(saved_lvl.par_moves, suggested.0);
    assert_eq!(saved_lvl.par_pipes, suggested.1);
    assert_eq!(saved_lvl.par_time_ms, suggested.2);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn editor_playtest_round_trip_and_unsaved_guard() {
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let root = std::env::temp_dir().join(format!("aqualux_playtest_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let primary = root.join("custom_packs");

    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let mut app =
        GameApp::new_with_custom_dir(bundle, Some(120), Some(&primary), test_gpu()).expect("init");
    let camp_records_before = app
        .save
        .active_profile()
        .map(|p| (p.records.len(), p.total_score()));

    app.open_custom_hub(false);
    app.trigger_action(UiAction::HubEditLevel);
    assert_eq!(app.screen, Screen::Editor);

    // Mark dirty via painting a side pipe
    app.trigger_action(UiAction::EditorSelectTool(EditorTool::Straight));
    assert!(app.editor.as_mut().unwrap().paint_cell(1, 0, true));
    assert!(app.editor.as_ref().unwrap().dirty);

    // Launch playtest
    app.trigger_action(UiAction::EditorTest);
    assert_eq!(app.screen, Screen::Playing);
    assert!(matches!(app.play_mode, PlayMode::EditorTest { .. }));

    // Winning playtest must not alter campaign or custom pack progress
    let res = calculate_score(4, 5, 5000, 6, 5, 15000, true);
    app.on_campaign_win(res);
    assert_eq!(app.screen, Screen::Summary);
    assert!(app.custom_packs.progress.packs.is_empty());
    assert_eq!(
        app.save
            .active_profile()
            .map(|p| (p.records.len(), p.total_score())),
        camp_records_before
    );

    app.advance_after_summary();
    assert_eq!(app.screen, Screen::Editor);
    assert_eq!(app.play_mode, PlayMode::Campaign);
    assert!(app.editor.as_ref().unwrap().dirty);

    // Pause -> Back to Editor round-trip
    app.trigger_action(UiAction::EditorTest);
    assert_eq!(app.screen, Screen::Playing);
    app.trigger_action(UiAction::PauseGame);
    assert_eq!(app.screen, Screen::Paused);
    app.trigger_action(UiAction::MainMenuPrompt);
    app.trigger_action(UiAction::QuestionYes);
    assert_eq!(app.screen, Screen::Editor);
    assert!(app.editor.as_ref().unwrap().dirty);

    // Removing the Drain prevents playtest launch
    app.trigger_action(UiAction::EditorSelectTool(EditorTool::Blocker));
    assert!(app.editor.as_mut().unwrap().paint_cell(4, 2, true));
    app.trigger_action(UiAction::EditorTest);
    assert_eq!(app.screen, Screen::Editor);
    assert!(!app.editor.as_ref().unwrap().validation_issues.is_empty());

    // Backing out while dirty triggers discard confirmation
    app.trigger_action(UiAction::EditorBack);
    assert!(matches!(
        app.modals.last(),
        Some(ModalState::Question {
            id: QuestionId::ConfirmDiscardEditor,
            ..
        })
    ));
    app.trigger_action(UiAction::QuestionYes);
    assert_eq!(app.screen, Screen::CustomHub);
    assert!(app.editor.is_none());

    let _ = fs::remove_dir_all(&root);
}

fn sample_pack(name: &str) -> CustomPackFile {
    CustomPackFile {
        meta: CustomPackMeta {
            name: name.to_string(),
            ..Default::default()
        },
        levels: vec![default_starter_level(0)],
    }
}

#[test]
fn pack_dir_and_delete_guards_block_unsafe_paths_and_tampered_files() {
    for bad in ["", "/", ".", "..", "foo/../bar", "/tmp/../etc"] {
        assert!(
            validate_pack_dir(Path::new(bad)).is_err(),
            "expected validate_pack_dir to reject {bad:?}"
        );
    }
    assert!(validate_pack_dir(Path::new("custom_packs")).is_ok());
    assert!(validate_pack_dir(Path::new("/tmp/aqualux_custom_packs")).is_ok());

    let root = std::env::temp_dir().join(format!("aqualux_guard_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let primary = root.join("primary");
    let outside = root.join("outside");
    let portable = root.join("portable");
    fs::create_dir_all(&primary).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::create_dir_all(&portable).unwrap();

    // Unsafe primary_dir rejected for save and folder open
    let mut unsafe_store =
        CustomPackStore::load_with_dirs(PathBuf::from("/"), Vec::new(), root.join("progress.json"));
    assert!(
        unsafe_store
            .save_pack("bad", sample_pack("Unsafe"))
            .is_err()
    );
    assert!(unsafe_store.open_packs_folder().is_err());

    // PortableDir pack refused even if writable bit is mutated in memory
    let portable_xml = portable.join("read-only.xml");
    fs::write(&portable_xml, write_pack_xml(&sample_pack("Portable"))).unwrap();

    let mut store = CustomPackStore::load_with_dirs(
        primary.clone(),
        vec![(portable.clone(), PackSource::PortableDir)],
        root.join("custom_progress.json"),
    );
    assert_eq!(store.packs.len(), 1);
    store.packs[0].writable = true;
    assert!(store.delete_pack("read-only").is_err());
    assert!(portable_xml.exists());

    // Tampered LoadedPack.path pointing to valid Aqualux XML outside primary_dir
    let outside_xml = outside.join("keep-me.xml");
    fs::write(&outside_xml, write_pack_xml(&sample_pack("Outside"))).unwrap();
    store.packs.push(LoadedPack {
        id: "tampered-outside".to_string(),
        path: outside_xml.clone(),
        source: PackSource::UserDir,
        writable: true,
        pack: sample_pack("Outside"),
    });
    assert!(store.delete_pack("tampered-outside").is_err());
    assert!(outside_xml.exists());

    // Tampered LoadedPack.path pointing to non-XML file inside primary_dir
    let notes_txt = primary.join("notes.txt");
    fs::write(&notes_txt, "important user notes").unwrap();
    store.packs.push(LoadedPack {
        id: "tampered-txt".to_string(),
        path: notes_txt.clone(),
        source: PackSource::UserDir,
        writable: true,
        pack: sample_pack("Txt"),
    });
    assert!(store.delete_pack("tampered-txt").is_err());
    assert!(notes_txt.exists());

    // Valid pack in primary_dir whose disk contents were replaced with non-Aqualux XML
    let victim_path = store
        .save_pack("victim", sample_pack("Victim"))
        .expect("save victim pack");
    fs::write(
        &victim_path,
        "<project><item name=\"unrelated\"/></project>",
    )
    .unwrap();
    assert!(store.delete_pack("victim").is_err());
    assert!(victim_path.exists());

    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn symlink_guards_block_delete_and_overwrite() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!("aqualux_symlink_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let primary = root.join("primary");
    let outside = root.join("outside");
    fs::create_dir_all(&primary).unwrap();
    fs::create_dir_all(&outside).unwrap();

    // 1. Symlink .xml inside primary_dir -> outside Aqualux .xml
    let outside_xml = outside.join("outside.xml");
    let outside_content = write_pack_xml(&sample_pack("Outside Original"));
    fs::write(&outside_xml, &outside_content).unwrap();
    let linked_xml = primary.join("linked.xml");
    symlink(&outside_xml, &linked_xml).unwrap();

    let mut store =
        CustomPackStore::load_with_dirs(primary.clone(), Vec::new(), root.join("progress.json"));
    assert!(
        !store.packs.iter().any(|p| p.id == "linked"),
        "scan_dir must ignore symlinked xml files"
    );
    assert!(store.save_pack("linked", sample_pack("Overwrite")).is_err());
    assert_eq!(fs::read_to_string(&outside_xml).unwrap(), outside_content);
    assert!(store.import_xml_file(&linked_xml).is_err());

    store.packs.push(LoadedPack {
        id: "linked".to_string(),
        path: linked_xml.clone(),
        source: PackSource::UserDir,
        writable: true,
        pack: sample_pack("Linked"),
    });
    assert!(store.delete_pack("linked").is_err());
    assert!(fs::symlink_metadata(&linked_xml).is_ok());
    assert!(outside_xml.exists());

    // 2. Symlink primary_dir -> real directory
    let real_primary = root.join("real_primary");
    fs::create_dir_all(&real_primary).unwrap();
    let real_pack = real_primary.join("kept.xml");
    fs::write(&real_pack, write_pack_xml(&sample_pack("Kept"))).unwrap();
    let primary_link = root.join("primary_link");
    symlink(&real_primary, &primary_link).unwrap();

    assert!(validate_pack_dir(&primary_link).is_err());
    let mut link_store = CustomPackStore::load_with_dirs(
        primary_link.clone(),
        Vec::new(),
        root.join("progress2.json"),
    );
    assert!(
        link_store
            .save_pack("new-pack", sample_pack("New"))
            .is_err()
    );
    link_store.packs.push(LoadedPack {
        id: "kept".to_string(),
        path: primary_link.join("kept.xml"),
        source: PackSource::UserDir,
        writable: true,
        pack: sample_pack("Kept"),
    });
    assert!(link_store.delete_pack("kept").is_err());
    assert!(real_pack.exists());

    // 3. Symlink progress_path -> external target
    let external_progress_target = outside.join("keep_progress.txt");
    fs::write(&external_progress_target, "do not overwrite progress").unwrap();
    let progress_link = root.join("progress_link.json");
    symlink(&external_progress_target, &progress_link).unwrap();
    let progress_store =
        CustomPackStore::load_with_dirs(primary.clone(), Vec::new(), progress_link);
    assert!(progress_store.save_progress_to_disk().is_err());
    assert_eq!(
        fs::read_to_string(&external_progress_target).unwrap(),
        "do not overwrite progress"
    );

    // 4. Symlink save.json -> external target
    let external_save_target = outside.join("keep_save.txt");
    fs::write(&external_save_target, "do not overwrite save").unwrap();
    let save_link = root.join("save_link.json");
    symlink(&external_save_target, &save_link).unwrap();
    assert!(SaveData::default().write_save_to_path(&save_link).is_err());
    assert_eq!(
        fs::read_to_string(&external_save_target).unwrap(),
        "do not overwrite save"
    );

    let _ = fs::remove_dir_all(&root);
}
