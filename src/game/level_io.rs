// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::levels::parse_levels_xml;
use crate::game::types::{GameColor, LevelDef, PieceShape, PieceType, TileDef};
use std::fmt::Write as _;

pub const MAX_CUSTOM_XML_BYTES: usize = 2 * 1024 * 1024;
pub const MIN_GRID_DIM: usize = 2;
pub const MAX_GRID_COLS: usize = 10;
pub const MAX_GRID_ROWS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CustomPackMeta {
    pub name: String,
    pub author: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CustomPackFile {
    pub meta: CustomPackMeta,
    pub levels: Vec<LevelDef>,
}

pub fn parse_pack_xml(xml_raw: &str, fallback_name: &str) -> Result<CustomPackFile, String> {
    if xml_raw.len() > MAX_CUSTOM_XML_BYTES {
        return Err(format!(
            "XML exceeds {} byte limit ({} bytes)",
            MAX_CUSTOM_XML_BYTES,
            xml_raw.len()
        ));
    }
    let trimmed = xml_raw.trim_start_matches('\u{feff}').trim();
    let doc = roxmltree::Document::parse(trimmed).map_err(|e| format!("XML parse error: {}", e))?;

    let mut meta = CustomPackMeta::default();
    if let Some(levels_node) = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "levels")
    {
        if let Some(n) = levels_node.attribute("name").map(str::trim)
            && !n.is_empty()
        {
            meta.name = n.to_string();
        }
        if let Some(a) = levels_node.attribute("author").map(str::trim) {
            meta.author = a.to_string();
        }
        if let Some(d) = levels_node.attribute("description").map(str::trim) {
            meta.description = d.to_string();
        }
    }
    if meta.name.is_empty() {
        let fb = fallback_name.trim();
        meta.name = if fb.is_empty() {
            "Custom Pack".to_string()
        } else {
            fb.to_string()
        };
    }

    let mut levels = parse_levels_xml(trimmed)?;
    if levels.is_empty() {
        return Err("XML file contains no levels".to_string());
    }

    for (i, lvl) in levels.iter_mut().enumerate() {
        lvl.index = i;
        lvl.cols = lvl.cols.clamp(MIN_GRID_DIM, MAX_GRID_COLS);
        lvl.rows = lvl.rows.clamp(MIN_GRID_DIM, MAX_GRID_ROWS);
        lvl.start_delay = if lvl.start_delay.is_finite() {
            lvl.start_delay.clamp(0.0, 120.0)
        } else {
            10.0
        };
        lvl.liquid_speed = if lvl.liquid_speed.is_finite() {
            lvl.liquid_speed.clamp(0.1, 20.0)
        } else {
            1.5
        };
        lvl.tiles.retain(|t| t.x < lvl.cols && t.y < lvl.rows);
    }

    Ok(CustomPackFile { meta, levels })
}

pub fn write_pack_xml(pack: &CustomPackFile) -> String {
    let mut out = String::with_capacity(4096);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        out,
        "<levels name=\"{}\" author=\"{}\" description=\"{}\">",
        escape_attr(&pack.meta.name),
        escape_attr(&pack.meta.author),
        escape_attr(&pack.meta.description),
    );
    for (idx, lvl) in pack.levels.iter().enumerate() {
        write_level_xml(&mut out, idx, lvl);
    }
    out.push_str("</levels>\n");
    out
}

fn write_level_xml(out: &mut String, idx: usize, lvl: &LevelDef) {
    let _ = writeln!(out, "  <level index=\"{}\">", idx);
    let _ = writeln!(out, "    <title>{}</title>", escape_text(&lvl.title));
    let _ = writeln!(
        out,
        "    <description>{}</description>",
        escape_text(&lvl.description)
    );
    out.push_str("    <levelData>\n");
    let _ = writeln!(
        out,
        "      <backgroundId>{}</backgroundId>",
        escape_text(&lvl.background_id)
    );
    let _ = writeln!(
        out,
        "      <liquidColor>{}</liquidColor>",
        color_to_xml(lvl.liquid_color)
    );
    let _ = writeln!(out, "      <startDelay>{}</startDelay>", lvl.start_delay);
    let _ = writeln!(out, "      <liquidSpeed>{}</liquidSpeed>", lvl.liquid_speed);
    let _ = writeln!(
        out,
        "      <rotaterColor>{}</rotaterColor>",
        color_to_xml(lvl.rotater_color)
    );
    let _ = writeln!(
        out,
        "      <moverColor>{}</moverColor>",
        color_to_xml(lvl.mover_color)
    );
    let _ = writeln!(
        out,
        "      <stationaryColor>{}</stationaryColor>",
        color_to_xml(lvl.stationary_color)
    );
    let _ = writeln!(
        out,
        "      <gateColor>{}</gateColor>",
        color_to_xml(lvl.gate_color)
    );
    let _ = writeln!(
        out,
        "      <freezeColor>{}</freezeColor>",
        color_to_xml(lvl.freeze_color)
    );
    let _ = writeln!(
        out,
        "      <magicColor>{}</magicColor>",
        color_to_xml(lvl.magic_color)
    );
    let _ = writeln!(
        out,
        "      <par moves=\"{}\" pipes=\"{}\" time=\"{}\"/>",
        lvl.par_moves, lvl.par_pipes, lvl.par_time_ms
    );
    if !lvl.tutorials.is_empty() {
        out.push_str("      <tutorial>\n");
        for tut in &lvl.tutorials {
            out.push_str("        <message");
            if !tut.tiles.is_empty() {
                let _ = write!(out, " tile=\"{}\"", format_point_list(&tut.tiles));
            }
            if let Some((sx, sy)) = tut.wait_for_select {
                let _ = write!(out, " waitForSelect=\"{},{}\"", sx, sy);
            }
            if !tut.wait_for_swap.is_empty() {
                let _ = write!(
                    out,
                    " waitForSwap=\"{}\"",
                    format_point_list(&tut.wait_for_swap)
                );
            }
            if !tut.wait_for_drag.is_empty() {
                let _ = write!(
                    out,
                    " waitForDrag=\"{}\"",
                    format_point_list(&tut.wait_for_drag)
                );
            }
            if tut.hold_hilight {
                out.push_str(" holdHilight=\"true\"");
            }
            let _ = writeln!(out, ">{}</message>", escape_text(&tut.text));
        }
        out.push_str("      </tutorial>\n");
    }
    let _ = writeln!(
        out,
        "      <tileGrid cols=\"{}\" rows=\"{}\">",
        lvl.cols, lvl.rows
    );
    for t in &lvl.tiles {
        let _ = writeln!(out, "        <tile x=\"{}\" y=\"{}\">", t.x, t.y);
        let _ = writeln!(out, "          <shape>{}</shape>", shape_to_xml(t.shape));
        let _ = writeln!(
            out,
            "          <type>{}</type>",
            piece_type_to_xml(t.piece_type)
        );
        let _ = writeln!(out, "          <direction>{}</direction>", t.direction % 4);
        if t.is_moveable {
            out.push_str("          <isMoveable/>\n");
        }
        if t.can_rotate {
            out.push_str("          <canRotate/>\n");
        }
        if t.is_gate {
            out.push_str("          <isGate/>\n");
        }
        if t.is_freeze {
            out.push_str("          <isFreeze/>\n");
        }
        if t.is_magic {
            out.push_str("          <isMagic/>\n");
        }
        if let Some(sc) = t.secondary_color {
            let _ = writeln!(
                out,
                "          <secondaryColor>{}</secondaryColor>",
                color_to_xml(sc)
            );
        }
        out.push_str("        </tile>\n");
    }
    out.push_str("      </tileGrid>\n");
    out.push_str("    </levelData>\n");
    out.push_str("  </level>\n");
}

pub fn shape_to_xml(s: PieceShape) -> &'static str {
    match s {
        PieceShape::Elbow => "elbow",
        PieceShape::Straight => "straight",
        PieceShape::Cross => "cross",
        PieceShape::Spigot => "spigot",
        PieceShape::Drain => "drain",
        PieceShape::Block => "block",
    }
}

pub fn piece_type_to_xml(p: PieceType) -> &'static str {
    match p {
        PieceType::Source => "source",
        PieceType::Pipe => "pipe",
        PieceType::Drain => "drain",
        PieceType::Blocker => "blocker",
        PieceType::Portal => "portal",
        PieceType::FrozenPipe => "frozenPipe",
    }
}

pub fn color_to_xml(c: GameColor) -> &'static str {
    match c {
        GameColor::Red => "RED",
        GameColor::Orange => "ORANGE",
        GameColor::Yellow => "YELLOW",
        GameColor::Green => "GREEN",
        GameColor::Blue => "BLUE",
        GameColor::Violet => "VIOLET",
        GameColor::White => "WHITE",
        GameColor::Grey => "GREY",
    }
}

pub fn default_starter_level(index: usize) -> LevelDef {
    let mut tiles = Vec::with_capacity(25);
    for y in 0..5 {
        for x in 0..5 {
            let tile = match (x, y) {
                // Spigot default port is Down (dir 0); dir 3 points Right -> (1, 2)
                (0, 2) => TileDef {
                    x,
                    y,
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
                // Drain default port is Down (dir 0); dir 1 points Left <- (3, 2)
                (4, 2) => TileDef {
                    x,
                    y,
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
                (1, 2) => pipe_tile(x, y, PieceShape::Elbow, 0, false, true),
                (2, 2) => pipe_tile(x, y, PieceShape::Straight, 0, false, true),
                (3, 2) => pipe_tile(x, y, PieceShape::Elbow, 1, true, false),
                (3, 1) => pipe_tile(x, y, PieceShape::Straight, 1, true, false),
                (0, 0) | (4, 0) | (0, 4) | (4, 4) => TileDef {
                    x,
                    y,
                    ..TileDef::dummy_blocker()
                },
                _ => pipe_tile(
                    x,
                    y,
                    if (x + y) % 2 == 0 {
                        PieceShape::Elbow
                    } else {
                        PieceShape::Straight
                    },
                    ((x + y) % 4) as u8,
                    true,
                    false,
                ),
            };
            tiles.push(tile);
        }
    }

    LevelDef {
        index,
        title: format!("Custom Level {}", index + 1),
        description: "Connect the source to the drain.".to_string(),
        background_id: "NightHills".to_string(),
        liquid_color: GameColor::Blue,
        start_delay: 10.0,
        liquid_speed: 1.5,
        rotater_color: GameColor::White,
        mover_color: GameColor::Yellow,
        stationary_color: GameColor::Orange,
        gate_color: GameColor::Violet,
        freeze_color: GameColor::Yellow,
        magic_color: GameColor::Green,
        par_moves: 6,
        par_pipes: 5,
        par_time_ms: 15000,
        cols: 5,
        rows: 5,
        tiles,
        tutorials: Vec::new(),
    }
}

fn pipe_tile(
    x: usize,
    y: usize,
    shape: PieceShape,
    direction: u8,
    is_moveable: bool,
    can_rotate: bool,
) -> TileDef {
    TileDef {
        x,
        y,
        shape,
        piece_type: PieceType::Pipe,
        direction: direction % 4,
        is_moveable,
        can_rotate,
        is_gate: false,
        is_freeze: false,
        is_magic: false,
        secondary_color: None,
    }
}

fn format_point_list(pts: &[(usize, usize)]) -> String {
    pts.iter()
        .map(|(x, y)| format!("{},{}", x, y))
        .collect::<Vec<_>>()
        .join("|")
}

fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}
