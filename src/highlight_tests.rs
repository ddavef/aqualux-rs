// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::{AssetBundle, RgbaImage};
use crate::game::scoring::calculate_score;
use crate::game::types::{LevelDef, TileDef, TutorialMsg};
use crate::game::{Difficulty, GameColor, LevelSim, PieceShape, PieceType, Side};
use crate::render::canvas::DrawCmd;
use crate::render::{Canvas, FontFamily, LOGICAL_W, Rect, Renderer, StarField, TexBank};
use crate::tests::{asset_file_if_present, empty_bundle, test_gpu};
use crate::ui::hud::draw_level_info_screen;
use crate::ui::summary::{SummaryHideAction, SummaryState, render_summary_screen};
use crate::ui::{GameApp, Screen};
use rand::SeedableRng;
use rand::rngs::StdRng;

fn td(
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
        is_freeze: false,
        is_magic: false,
        secondary_color: None,
    }
}

fn hl_test_level(can_rotate_pipe: bool, tutorials: Vec<TutorialMsg>) -> LevelDef {
    LevelDef {
        index: 0,
        title: "HL".into(),
        description: String::new(),
        background_id: "DaySky".into(),
        liquid_color: GameColor::Blue,
        start_delay: 20.0,
        liquid_speed: 6.0,
        rotater_color: GameColor::White,
        mover_color: GameColor::Yellow,
        stationary_color: GameColor::Orange,
        gate_color: GameColor::Green,
        freeze_color: GameColor::White,
        magic_color: GameColor::Violet,
        par_moves: 1,
        par_pipes: 3,
        par_time_ms: 10_000,
        cols: 3,
        rows: 2,
        tiles: vec![
            td(0, 0, PieceShape::Spigot, PieceType::Source, 3, false, false),
            td(
                1,
                0,
                PieceShape::Straight,
                PieceType::Pipe,
                0,
                !can_rotate_pipe,
                can_rotate_pipe,
            ),
            td(2, 0, PieceShape::Drain, PieceType::Drain, 1, false, false),
            td(0, 1, PieceShape::Straight, PieceType::Pipe, 0, true, false),
            td(1, 1, PieceShape::Straight, PieceType::Pipe, 1, true, false),
            td(2, 1, PieceShape::Elbow, PieceType::Pipe, 0, true, false),
        ],
        tutorials,
    }
}

#[test]
fn per_tile_green_path_highlight_ownership_timing_and_tutorial_hold() {
    // 1. Swap to connect path -> local coords + 1.5s fade, unrelated swap preserves ongoing fade,
    //    and swapping a glowing tile moves path_hl with the tile instead of jumping to its neighbor.
    let mut sim = LevelSim::new(hl_test_level(false, Vec::new()), Difficulty::Medium);
    sim.start_level();
    assert!(sim.tiles[0][0].path_hl.is_none());

    let _ = sim.click_tile(1, 1);
    let _ = sim.click_tile(1, 0);
    let mut evs = sim.update(0.25);
    evs.extend(sim.update(0.20));
    assert!(
        evs.iter()
            .any(|e| matches!(e, crate::game::SimEvent::PlaySfx("SwishSound")))
    );
    assert!(sim.tiles[0][0].path_hl.is_some());
    assert!(sim.tiles[0][1].path_hl.is_some());
    assert!(sim.tiles[0][2].path_hl.is_some());
    assert!(sim.tiles[1][1].path_hl.is_none());

    // Advance into hold window (t ~ 0.35s) and swap two unrelated bottom tiles.
    let _ = sim.update(0.30);
    assert!((sim.tiles[0][1].path_hl.as_ref().unwrap().alpha() - 1.0).abs() < 1e-4);
    let _ = sim.click_tile(0, 1);
    let _ = sim.click_tile(1, 1);
    let mut ev2 = sim.update(0.25);
    ev2.extend(sim.update(0.20));
    assert!(
        !ev2.iter()
            .any(|e| matches!(e, crate::game::SimEvent::PlaySfx("SwishSound")))
    );
    // Total elapsed since Swish ~ 0.8s -> starts fading (0.8..1.5s), not reset or erased.
    assert!(sim.tiles[0][1].path_hl.is_some());
    let _ = sim.update(0.35);
    let mid_a = sim.tiles[0][1].path_hl.as_ref().unwrap().alpha();
    assert!(
        mid_a > 0.1 && mid_a < 0.7,
        "expected mid-fade alpha, got {}",
        mid_a
    );

    // Drag glowing tile (1,0) into (1,1) mid-fade: highlight must move WITH the tile immediately.
    let _ = sim.try_swap_drag((1, 0), (1, 1));
    assert!(
        sim.tiles[0][1].path_hl.is_none(),
        "swapped neighbor must not inherit highlight"
    );
    assert!(
        sim.tiles[1][1].path_hl.is_some(),
        "moved tile must keep its highlight"
    );
    let _ = sim.update(0.8);
    assert!(
        sim.tiles[1][1].path_hl.is_none(),
        "fade must expire after 1.5s total"
    );

    // 2. Rotater tile stores local segment coords so highlight rotates with visual_rot_deg.
    let mut rsim = LevelSim::new(hl_test_level(true, Vec::new()), Difficulty::Medium);
    rsim.start_level();
    let _ = rsim.click_tile(1, 0);
    let _ = rsim.update(0.45);
    let rhl = rsim.tiles[0][1]
        .path_hl
        .as_ref()
        .expect("rotated tile should highlight");
    assert_eq!(rhl.segs, vec![(Side::Down, Side::Up)]);

    // 3. Level 1-style hold_hilight tutorial: stays dark BEFORE move, holds through congratulatory next message,
    //    and clears when dismissing the congratulatory message.
    let tuts = vec![
        TutorialMsg {
            tiles: vec![(0, 0)],
            wait_for_select: None,
            wait_for_swap: vec![],
            wait_for_drag: vec![],
            hold_hilight: false,
            text: "Step 0".into(),
        },
        TutorialMsg {
            tiles: vec![(1, 0)],
            wait_for_select: None,
            wait_for_swap: vec![(1, 1), (1, 0)],
            wait_for_drag: vec![],
            hold_hilight: true,
            text: "Swap".into(),
        },
        TutorialMsg {
            tiles: vec![(1, 0)],
            wait_for_select: None,
            wait_for_swap: vec![],
            wait_for_drag: vec![],
            hold_hilight: false,
            text: "Great".into(),
        },
    ];
    let mut tsim = LevelSim::new(hl_test_level(false, tuts), Difficulty::Medium);
    tsim.start_level();
    let _ = tsim.next_tutorial();
    assert!(
        tsim.tiles[0][0].path_hl.is_none(),
        "path must stay unlit before completing tutorial swap"
    );

    let _ = tsim.click_tile(1, 1);
    let _ = tsim.click_tile(1, 0);
    let _ = tsim.update(0.45);
    assert_eq!(tsim.tutorial_idx, 2);
    assert!(tsim.tiles[0][1].path_hl.as_ref().is_some_and(|h| h.hold));
    for _ in 0..6 {
        let _ = tsim.update(0.5);
    }
    assert!((tsim.tiles[0][1].path_hl.as_ref().unwrap().alpha() - 1.0).abs() < 1e-4);

    let _ = tsim.next_tutorial();
    assert!(tsim.tiles[0][0].path_hl.is_none() && tsim.tiles[0][1].path_hl.is_none());
}

#[test]
fn summary_hide_labels_and_level_info_clear_viewport() {
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let gpu = test_gpu();
    let renderer = Renderer::new(gpu.clone(), &bundle).expect("renderer");

    let mut sum = SummaryState::new(calculate_score(4, 5, 6000, 4, 5, 8000, true), 1000, 2400);
    sum.age = 2.25;
    sum.start_hide(SummaryHideAction::NextLevel);
    sum.hiding = Some((1.35, SummaryHideAction::NextLevel));
    let mut canvas = Canvas::new();
    canvas.begin_frame(1.0);
    render_summary_screen(
        &mut canvas,
        &renderer.bank,
        &renderer.fonts,
        &sum,
        (0.0, 0.0),
    );
    let has_visible_label = canvas.cmds.iter().any(|cmd| match cmd {
        DrawCmd::SpriteDirect(_, inst) => {
            inst.row1[2] >= 260.0
                && inst.row1[2] <= 460.0
                && inst.row0[3] > 80.0
                && inst.row0[2] < 800.0
        }
        _ => false,
    });
    assert!(
        !has_visible_label,
        "All Summary score labels must have slid off x >= 800 by h=1.35s"
    );

    sum.hiding = Some((1.85, SummaryHideAction::NextLevel));
    canvas.begin_frame(1.0);
    render_summary_screen(
        &mut canvas,
        &renderer.bank,
        &renderer.fonts,
        &sum,
        (0.0, 0.0),
    );
    let has_any_row_text = canvas.cmds.iter().any(|cmd| match cmd {
        DrawCmd::SpriteDirect(_, inst) => {
            inst.row1[2] >= 260.0 && inst.row1[2] <= 460.0 && inst.row0[2] < 800.0
        }
        _ => false,
    });
    assert!(
        !has_any_row_text,
        "All Summary labels and counters must be gone by h=1.85s"
    );

    let mut info_mid = Canvas::new();
    info_mid.begin_frame(1.0);
    info_mid.clear((0, 0, 0));
    draw_level_info_screen(
        &mut info_mid,
        &renderer.fonts,
        "Very Long Level Title Name Here",
        "Very long level subtitle description text",
        4.4,
        0.0,
    );
    let px_mid = gpu.render_to_rgba(&info_mid, &renderer.bank, LOGICAL_W as u32, 600);
    assert!(
        px_mid
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0)
    );

    let mut info_c = Canvas::new();
    info_c.begin_frame(1.0);
    info_c.clear((0, 0, 0));
    draw_level_info_screen(
        &mut info_c,
        &renderer.fonts,
        "Very Long Level Title Name Here",
        "Very long level subtitle description text",
        4.8,
        0.0,
    );
    let px = gpu.render_to_rgba(&info_c, &renderer.bank, LOGICAL_W as u32, 600);
    assert!(
        px.as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[0] == 0 && p[1] == 0 && p[2] == 0)
    );
}

#[test]
fn gpu_expanded_quad_gaussian_blur_smoothness_and_no_clipping() {
    let gpu = test_gpu();
    let mut b = empty_bundle();
    b.images.insert(
        "white_sq".into(),
        RgbaImage {
            width: 64,
            height: 64,
            data: vec![255u8; 64 * 64 * 4],
        },
    );
    let bank = TexBank::new(gpu.clone(), &b);
    let tid = bank.named("white_sq").unwrap().0;

    let mut c2d = Canvas::new();
    c2d.begin_frame(1.0);
    c2d.clear((0, 0, 0));
    c2d.draw_tex_rect_blur(
        tid,
        Rect::new(368.0, 268.0, 64.0, 64.0),
        None,
        1.0,
        1.0,
        (16.0, 16.0),
    );
    let px2d = gpu.render_to_rgba(&c2d, &bank, 800, 600);
    let r_at = |buf: &[u8], x: usize, y: usize| buf[(y * 800 + x) * 4];

    assert!(
        r_at(&px2d, 434, 300) > 40,
        "2D blur halo must expand beyond original quad bounds"
    );
    assert!(r_at(&px2d, 433, 300) >= r_at(&px2d, 435, 300));
    assert!(r_at(&px2d, 435, 300) >= r_at(&px2d, 438, 300));
    assert!(r_at(&px2d, 438, 300) >= r_at(&px2d, 444, 300));

    let mut c1d = Canvas::new();
    c1d.begin_frame(1.0);
    c1d.clear((0, 0, 0));
    c1d.draw_tex_rect_blur(
        tid,
        Rect::new(368.0, 268.0, 64.0, 64.0),
        None,
        1.0,
        1.0,
        (24.0, 0.0),
    );
    let px1d = gpu.render_to_rgba(&c1d, &bank, 800, 600);
    assert!(
        r_at(&px1d, 436, 300) > 30,
        "Horizontal blur must extend horizontally past quad bounds"
    );
    assert_eq!(
        r_at(&px1d, 400, 265),
        0,
        "Horizontal-only blur must have zero vertical bleed"
    );
}

#[test]
fn champion_screen_starfield_pipes_orbs_backdrop_and_layout_parity() {
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let gpu = test_gpu();
    let mut app = GameApp::new(bundle, Some(120), gpu.clone()).expect("GameApp init");
    app.start_campaign_level(99, false);
    assert_eq!(app.bg_anim.bg_id, "OuterSpace");
    app.bg_anim.darken = 1.0;

    app.enter_champion_screen(123_456, false);
    assert_eq!(app.screen, Screen::Champion);

    let w_grand = app
        .renderer
        .fonts
        .measure_width(FontFamily::Neuropol, "GRAND", 46.0);
    let w_champ = app
        .renderer
        .fonts
        .measure_width(FontFamily::Neuropol, "CHAMPION!", 46.0);
    assert!(
        45.0 + 1.5 * w_grand < 210.0 + 0.5 * w_champ,
        "GRAND must not overlap CHAMPION!"
    );
    assert!(
        210.0 + 1.5 * w_champ <= 795.0,
        "CHAMPION! must fit within 800px viewport"
    );

    {
        let ch = app.champion_state.as_mut().unwrap();
        ch.age = 8.8;
        ch.star_field = StarField::new(&mut StdRng::seed_from_u64(7));
    }

    let space_tid = app.renderer.bank.named("SpaceSkyBitmap").unwrap().0;
    let mut canvas = Canvas::new();
    let mut found_orb_and_pipe = false;
    for _ in 0..30 {
        canvas.begin_frame(1.0);
        app.render(&mut canvas);
        let has_orb = canvas.cmds.iter().any(
            |c| matches!(c, DrawCmd::Shape(s) if (s.meta[0] - 3.0).abs() < 0.1 && s.meta[3] > 1.5),
        );
        let has_pipe = canvas
            .cmds
            .iter()
            .any(|c| matches!(c, DrawCmd::SpriteTex(_, inst) if inst.params[0] > 0.0));
        if has_orb && has_pipe {
            found_orb_and_pipe = true;
            break;
        }
        app.champion_state.as_mut().unwrap().star_field.update(0.1);
    }
    assert!(
        found_orb_and_pipe,
        "Champion StarField must emit both radial star orbs and tinted flying pipes"
    );

    assert!(
        !canvas
            .cmds
            .iter()
            .any(|c| matches!(c, DrawCmd::Clear(0, 0, 0)))
    );
    assert!(
        canvas
            .cmds
            .iter()
            .any(|c| matches!(c, DrawCmd::SpriteTex(tid, _) if *tid == space_tid))
    );
    assert!(
        canvas
            .cmds
            .iter()
            .any(|c| matches!(c, DrawCmd::SpriteTex(_, inst) if inst.params[1] < -0.5))
    );
    assert!(
        canvas
            .cmds
            .iter()
            .any(|c| matches!(c, DrawCmd::SpriteDirect(_, inst) if inst.params[1] < -0.5))
    );

    let px = gpu.render_to_rgba(&canvas, &app.renderer.bank, LOGICAL_W as u32, 600);
    assert!(
        px.as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0)
    );
}
