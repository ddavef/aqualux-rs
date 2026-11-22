// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::AssetBundle;
use crate::audio::music_for_background;
use crate::game::scoring::{calculate_score, generate_match_level};
use crate::game::types::{LevelDef, TileDef};
use crate::game::{
    Difficulty, GameColor, LevelSim, MatchSim, PieceShape, PieceType, Side, SimEvent, Tile, Trophy,
    parse_levels_xml,
};
use crate::render::{Canvas, GpuCore, LOGICAL_H, LOGICAL_W, LeakEmitter, TexBank};
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

pub(crate) fn test_gpu() -> Arc<GpuCore> {
    static GPU: OnceLock<Arc<GpuCore>> = OnceLock::new();
    GPU.get_or_init(|| Arc::new(GpuCore::new_headless().expect("Headless GPU required for tests")))
        .clone()
}

pub(crate) fn empty_bundle() -> AssetBundle {
    AssetBundle {
        source_label: "test".into(),
        images: HashMap::new(),
        cid_images: HashMap::new(),
        sprites: HashMap::new(),
        fonts: HashMap::new(),
        sounds: HashMap::new(),
        raw_xmls: HashMap::new(),
        campaign_levels: Vec::new(),
        lotd_levels: Vec::new(),
    }
}

pub(crate) fn asset_file_if_present() -> Option<&'static Path> {
    [
        Path::new("resource.n"),
        Path::new("../resource.n"),
        Path::new("../aqualux.swf"),
    ]
    .into_iter()
    .find(|p| p.exists())
}

fn minimal_tile(
    x: usize,
    y: usize,
    shape: PieceShape,
    piece_type: PieceType,
    direction: u8,
    can_rotate: bool,
) -> TileDef {
    TileDef {
        x,
        y,
        shape,
        piece_type,
        direction,
        is_moveable: false,
        can_rotate,
        is_gate: false,
        is_freeze: false,
        is_magic: false,
        secondary_color: None,
    }
}

pub(crate) fn minimal_test_level() -> LevelDef {
    LevelDef {
        index: 0,
        title: "Test".into(),
        description: "Desc".into(),
        background_id: "DaySky".into(),
        liquid_color: GameColor::Blue,
        start_delay: 0.0,
        liquid_speed: 6.0,
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
            minimal_tile(0, 0, PieceShape::Spigot, PieceType::Source, 3, false),
            minimal_tile(1, 0, PieceShape::Straight, PieceType::Pipe, 0, true),
            minimal_tile(2, 0, PieceShape::Drain, PieceType::Drain, 1, false),
        ],
        tutorials: Vec::new(),
    }
}

#[test]
fn assets_require_real_files_with_no_fallback() {
    let err = match AssetBundle::load_from_path(Path::new("/nonexistent/aqualux_missing_path")) {
        Ok(_) => panic!("missing asset path must fail without procedural fallback"),
        Err(e) => e,
    };
    assert!(err.contains("Failed to read") || err.contains("Unsupported"));
}

#[test]
fn music_selection_matches_background_id() {
    assert_eq!(music_for_background("NightHills"), "GlideMusic");
    assert_eq!(music_for_background("DaySky"), "InGameLittleRayMusic");
    assert_eq!(music_for_background("bluesky"), "InGameLittleRayMusic");
    assert_eq!(music_for_background("SnowyHills"), "IstanbulMusic");
    assert_eq!(music_for_background("OuterSpace"), "DriftingMusic");
    assert_eq!(music_for_background("UnknownBg"), "GlideMusic");
}

#[test]
fn tile_ports_and_timed_rotation() {
    let elbow = Tile::from_def(
        &TileDef {
            x: 0,
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
        &minimal_test_level(),
    );
    assert_eq!(elbow.ports, [true, true, false, false]);
    assert_eq!(elbow.get_exit_side(Side::Up), Some(Side::Right));
    let mut grid = vec![vec![elbow]];
    grid[0][0].rotate_cw_timed(1, 0.4);
    assert_eq!(grid[0][0].ports, [false, true, true, false]);
    assert_eq!(grid[0][0].get_exit_side(Side::Right), Some(Side::Down));
    assert!(grid[0][0].is_acting());
    assert!(!crate::game::tile::animate_grid_tiles(
        &mut grid,
        0.2,
        &[(0, 0)]
    ));
    assert!(crate::game::tile::animate_grid_tiles(
        &mut grid,
        0.25,
        &[(0, 0)]
    ));
    assert!(!grid[0][0].is_acting());
}

#[test]
fn metric_calculator_par_over_par_and_platinum() {
    let hard_par = calculate_score(4, 10, 15_000, 4, 10, 15_000, true);
    assert_eq!(hard_par.trophy, Trophy::Platinum);
    assert_eq!(hard_par.pipe_bonus, 1500);
    assert_eq!(hard_par.time_bonus, 600);
    assert_eq!(hard_par.move_bonus, 300);

    let med_par = calculate_score(4, 10, 15_000, 4, 10, 15_000, false);
    assert_eq!(med_par.trophy, Trophy::Gold);
    assert_eq!(med_par.total_score, 1600);

    let over_par = calculate_score(10, 6, 40_000, 5, 6, 20_000, false);
    assert!(over_par.move_bonus < 200);
    assert!(over_par.time_bonus < 400);
}

#[test]
fn level_sim_speed_scaling_and_leak_event() {
    let mut delay_def = minimal_test_level();
    delay_def.start_delay = 10.0;

    let mut sim_easy = LevelSim::new(delay_def.clone(), Difficulty::Easy);
    let mut sim_med = LevelSim::new(delay_def.clone(), Difficulty::Medium);
    let mut sim_hard = LevelSim::new(delay_def.clone(), Difficulty::Hard);
    let expected_speed = delay_def.liquid_speed * 0.5;
    assert!((sim_easy.current_speed - expected_speed).abs() < 1e-4);
    assert!((sim_med.current_speed - expected_speed).abs() < 1e-4);
    assert!((sim_hard.current_speed - expected_speed).abs() < 1e-4);

    let _ = sim_easy.start_level_play();
    let _ = sim_med.start_level_play();
    let _ = sim_hard.start_level_play();
    assert!(sim_easy.tiles[0][0].timer.is_none());
    assert!((sim_med.tiles[0][0].timer.as_ref().unwrap().remaining - 30.0).abs() < 1e-4);
    assert!((sim_hard.tiles[0][0].timer.as_ref().unwrap().remaining - 10.0).abs() < 1e-4);

    let def = minimal_test_level();
    // Tile (1,0) starts vertical, so flowing Right out of Source hits closed Left port -> LeakErupt
    let mut sim = LevelSim::new(def, Difficulty::Hard);
    assert!(
        sim.update(1.0).is_empty(),
        "must not tick before start_level()"
    );
    let _ = sim.start_level();
    let mut saw_leak = false;
    for _ in 0..400 {
        for ev in sim.update(1.0 / 60.0) {
            if matches!(ev, SimEvent::LeakErupt(0, 0, Side::Right, GameColor::Blue)) {
                saw_leak = true;
            }
        }
        if sim.finished {
            break;
        }
    }
    assert!(saw_leak);
    assert_eq!(sim.won, Some(false));
}

#[test]
fn simulation_is_frame_rate_independent_30_120_240() {
    let mut def = minimal_test_level();
    def.tiles[1].direction = 1; // horizontal right away -> connects Source -> Pipe -> Drain
    let mut durations = Vec::new();
    for fps in [30usize, 120usize, 240usize] {
        let mut sim = LevelSim::new(def.clone(), Difficulty::Medium);
        sim.start_level();
        let dt = 1.0 / fps as f32;
        for _ in 0..(fps * 30) {
            sim.update(dt);
            if sim.finished {
                break;
            }
        }
        assert_eq!(sim.won, Some(true));
        durations.push(sim.elapsed_ms);
    }
    assert!(
        (durations[0] - durations[1]).abs() < 45.0,
        "30 vs 120Hz drift: {:?}",
        durations
    );
    assert!(
        (durations[1] - durations[2]).abs() < 20.0,
        "120 vs 240Hz drift: {:?}",
        durations
    );
}

#[test]
fn match_and_mirror_sim_complete() {
    let mut rng = StdRng::seed_from_u64(42);
    for mirror in [false, true] {
        let mut msim = MatchSim::new(8, mirror, &mut rng);
        assert!(
            msim.click_tile(0, 0).is_empty(),
            "cannot act before start_level()"
        );
        msim.start_level();
        msim.play_tiles = msim.goal_tiles.clone();
        assert!(msim.is_solved());
        assert_eq!(msim.mirror_mode, mirror);
    }

    let mut rng2 = StdRng::seed_from_u64(7);
    let m10 = generate_match_level(10, &mut rng2);
    assert!(m10.cols >= 4 && m10.rows >= 4);
}

#[test]
fn leak_emitter_fixed_30hz_step_matches_high_fps() {
    let mut emit_low = LeakEmitter::new((0, 0), 200.0, Side::Right, GameColor::Blue);
    let mut emit_high = LeakEmitter::new((0, 0), 200.0, Side::Right, GameColor::Blue);
    for _ in 0..10 {
        emit_low.update(0.035);
    }
    for _ in 0..80 {
        emit_high.update(0.035 / 8.0);
    }
    assert_eq!(emit_low.tick_count(), emit_high.tick_count());
    assert_eq!(emit_low.particle_count(), emit_high.particle_count());
    let gpu = test_gpu();
    let bank = TexBank::new(gpu.clone(), &empty_bundle());
    let mut c30 = Canvas::new();
    let mut c240 = Canvas::new();
    c30.clear((0, 0, 0));
    c240.clear((0, 0, 0));
    emit_low.draw_at(&mut c30, 200.0, 200.0);
    emit_high.draw_at(&mut c240, 200.0, 200.0);
    let px30 = gpu.render_to_rgba(&c30, &bank, LOGICAL_W as u32, LOGICAL_H as u32);
    let px240 = gpu.render_to_rgba(&c240, &bank, LOGICAL_W as u32, LOGICAL_H as u32);
    assert_eq!(px30, px240);
    assert!(
        px30.as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[0] > 20 || p[1] > 20 || p[2] > 20),
        "GPU LeakEmitter must render non-empty beveled stream"
    );
}

#[test]
fn xml_multi_tile_tutorial_parsing() {
    let xml = r#"<levels><level index="0"><title>Tut</title><description>D</description><levelData>
    <backgroundId>DaySky</backgroundId><liquidColor>BLUE</liquidColor><startDelay>0</startDelay><liquidSpeed>5</liquidSpeed>
    <rotaterColor>GREEN</rotaterColor><moverColor>ORANGE</moverColor><stationaryColor>GREY</stationaryColor>
    <gateColor>WHITE</gateColor><freezeColor>BLUE</freezeColor><magicColor>VIOLET</magicColor>
    <par moves="1" pipes="2" time="5000"/>
    <tileGrid cols="2" rows="1">
        <tile x="0" y="0"><type>Source</type><shape>Spigot</shape><direction>3</direction></tile>
        <tile x="1" y="0"><type>Drain</type><shape>Drain</shape><direction>3</direction></tile>
    </tileGrid>
    <tutorial>
        <message tile="0,0|1,0" waitForSwap="0,0|1,0" holdHilight="true">Line one&lt;br&gt;Line two</message>
    </tutorial>
    </levelData></level></levels>"#;
    let lvls = parse_levels_xml(xml).unwrap();
    assert_eq!(lvls.len(), 1);
    assert_eq!(lvls[0].tutorials[0].tiles, vec![(0, 0), (1, 0)]);
    assert_eq!(lvls[0].tutorials[0].wait_for_swap, vec![(0, 0), (1, 0)]);
    assert!(lvls[0].tutorials[0].hold_hilight);
    assert_eq!(lvls[0].tutorials[0].text, "Line one\nLine two");
}
