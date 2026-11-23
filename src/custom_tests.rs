// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::AssetBundle;
use crate::custom::{CustomPackStore, PackSource, sanitize_pack_id};
use crate::game::generator::{GenDifficulty, generate_custom_level};
use crate::game::level_io::{
    CustomPackFile, CustomPackMeta, MAX_CUSTOM_XML_BYTES, default_starter_level, parse_pack_xml,
    write_pack_xml,
};
use crate::game::scoring::calculate_score;
use crate::game::solver::{SolveOutcome, solve_level, validate_level_structure};
use crate::game::types::{GameColor, LevelDef, PieceShape, PieceType, TileDef, TutorialMsg};
use crate::tests::asset_file_if_present;
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::fs;
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
