// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::types::{GameColor, LevelDef, PieceShape, PieceType, TileDef, TutorialMsg};

pub fn parse_levels_xml(xml_raw: &str) -> Result<Vec<LevelDef>, String> {
    let trimmed = xml_raw.trim_start_matches('\u{feff}').trim();
    let doc = roxmltree::Document::parse(trimmed).map_err(|e| format!("XML parse error: {}", e))?;

    let mut levels = Vec::new();
    for (idx, level_node) in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "level")
        .enumerate()
    {
        if let Some(lvl) = parse_single_level(idx, level_node) {
            levels.push(lvl);
        }
    }

    levels.sort_by_key(|l| l.index);
    Ok(levels)
}

fn parse_single_level(idx: usize, level_node: roxmltree::Node<'_, '_>) -> Option<LevelDef> {
    let index = level_node
        .attribute("index")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(idx);
    let title = child_text(&level_node, "title").unwrap_or_else(|| format!("Level {}", index + 1));
    let description = child_text(&level_node, "description").unwrap_or_default();

    let ld = level_node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "levelData")?;

    let background_id = normalize_bg_id(
        &child_text(&ld, "backgroundId").unwrap_or_else(|| "NightHills".to_string()),
    );
    let liquid_color = child_color(&ld, "liquidColor", GameColor::Blue);
    let start_delay = child_text(&ld, "startDelay")
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(10.0);
    let liquid_speed = child_text(&ld, "liquidSpeed")
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(1.5);

    let rotater_color = child_color(&ld, "rotaterColor", GameColor::Blue);
    let mover_color = child_color(&ld, "moverColor", GameColor::Yellow);
    let stationary_color = child_color(&ld, "stationaryColor", GameColor::Orange);
    let gate_color = child_color(&ld, "gateColor", GameColor::Green);
    let freeze_color = child_color(&ld, "freezeColor", GameColor::White);
    let magic_color = child_color(&ld, "magicColor", GameColor::Violet);

    let (par_moves, par_pipes, par_time_ms) = parse_par(&ld);
    let tutorials = parse_tutorials(&ld);

    let grid_node = ld
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "tileGrid")?;
    let cols = grid_node
        .attribute("cols")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(5);
    let rows = grid_node
        .attribute("rows")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(5);

    let mut tiles = Vec::new();
    for tnode in grid_node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "tile")
    {
        let piece_type = child_text(&tnode, "type")
            .and_then(|s| PieceType::from_str_loose(&s))
            .unwrap_or(PieceType::Pipe);
        tiles.push(TileDef {
            x: tnode
                .attribute("x")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            y: tnode
                .attribute("y")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            shape: child_text(&tnode, "shape")
                .and_then(|s| PieceShape::from_str_loose(&s))
                .unwrap_or(PieceShape::Elbow),
            piece_type,
            direction: child_text(&tnode, "direction")
                .and_then(|s| s.trim().parse::<u8>().ok())
                .unwrap_or(0)
                % 4,
            is_moveable: has_child(&tnode, "isMoveable"),
            can_rotate: has_child(&tnode, "canRotate"),
            is_gate: has_child(&tnode, "isGate"),
            is_freeze: piece_type == PieceType::FrozenPipe || has_child(&tnode, "isFreeze"),
            is_magic: has_child(&tnode, "isMagic"),
            secondary_color: child_text(&tnode, "secondaryColor")
                .and_then(|s| GameColor::from_str_loose(&s)),
        });
    }

    Some(LevelDef {
        index,
        title,
        description,
        background_id,
        liquid_color,
        start_delay,
        liquid_speed,
        rotater_color,
        mover_color,
        stationary_color,
        gate_color,
        freeze_color,
        magic_color,
        par_moves,
        par_pipes,
        par_time_ms,
        cols,
        rows,
        tiles,
        tutorials,
    })
}

fn parse_par(ld: &roxmltree::Node<'_, '_>) -> (u32, u32, u32) {
    let mut moves = 10u32;
    let mut pipes = 5u32;
    let mut time_ms = 10000u32;
    if let Some(par_node) = ld
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "par")
    {
        if let Some(v) = par_node.attribute("moves").and_then(|s| s.parse().ok()) {
            moves = v;
        }
        if let Some(v) = par_node.attribute("pipes").and_then(|s| s.parse().ok()) {
            pipes = v;
        }
        if let Some(v) = par_node.attribute("time").and_then(|s| s.parse().ok()) {
            time_ms = v;
        }
    }
    (moves, pipes, time_ms)
}

fn parse_point_list(raw: Option<&str>) -> Vec<(usize, usize)> {
    let Some(s) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in s.split('|') {
        let mut parts = item.trim().split(',');
        if let (Some(x), Some(y)) = (
            parts.next().and_then(|v| v.trim().parse::<usize>().ok()),
            parts.next().and_then(|v| v.trim().parse::<usize>().ok()),
        ) {
            out.push((x, y));
        }
    }
    out
}

fn parse_tutorials(ld: &roxmltree::Node<'_, '_>) -> Vec<TutorialMsg> {
    let mut tutorials = Vec::new();
    let Some(tut_node) = ld
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "tutorial")
    else {
        return tutorials;
    };
    let mut hold_hilight = false;
    for msg in tut_node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "message")
    {
        if msg.attribute("holdHilight") == Some("true") {
            hold_hilight = true;
        }
        let tiles = parse_point_list(msg.attribute("tile"));
        let wait_for_select = parse_point_list(msg.attribute("waitForSelect"))
            .into_iter()
            .next();
        let wait_for_swap = parse_point_list(msg.attribute("waitForSwap"));
        let wait_for_drag = parse_point_list(msg.attribute("waitForDrag"));
        tutorials.push(TutorialMsg {
            tiles,
            wait_for_select,
            wait_for_swap,
            wait_for_drag,
            hold_hilight,
            text: strip_html_tags(msg.text().unwrap_or("")),
        });
    }
    tutorials
}

fn child_color(node: &roxmltree::Node<'_, '_>, tag: &str, default: GameColor) -> GameColor {
    child_text(node, tag)
        .and_then(|s| GameColor::from_str_loose(&s))
        .unwrap_or(default)
}

fn child_text(node: &roxmltree::Node<'_, '_>, tag: &str) -> Option<String> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == tag)
        .and_then(|n| n.text())
        .map(|s| s.trim().to_string())
}

fn has_child(node: &roxmltree::Node<'_, '_>, tag: &str) -> bool {
    node.children()
        .any(|n| n.is_element() && n.tag_name().name() == tag)
}

fn normalize_bg_id(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "nighthills" => "NightHills".to_string(),
        "daysky" | "bluesky" => "DaySky".to_string(),
        "snowyhills" | "snowhills" => "SnowyHills".to_string(),
        "outerspace" | "spacesky" => "OuterSpace".to_string(),
        other if !other.is_empty() => raw.trim().to_string(),
        _ => "NightHills".to_string(),
    }
}

pub fn strip_html_tags(input: &str) -> String {
    let with_breaks = input
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<BR>", "\n")
        .replace('\r', "\n");
    let mut out = String::with_capacity(with_breaks.len());
    let mut in_tag = false;
    for ch in with_breaks.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split('\n')
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}
