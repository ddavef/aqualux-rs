// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::types::{GameColor, LevelDef, PieceShape, PieceType, TileDef, TutorialMsg};
use quick_xml::de::from_str as xml_from_str;
use quick_xml::se::Serializer;
use serde::{Deserialize, Deserializer, Serialize, Serializer as SerdeSerializer};

pub const MAX_CUSTOM_XML_BYTES: usize = 2 * 1024 * 1024;
pub const MIN_GRID_DIM: usize = 2;
pub const MAX_GRID_COLS: usize = 10;
pub const MAX_GRID_ROWS: usize = 8;
pub const MAX_START_DELAY: f32 = 99.0;
pub const MIN_LIQUID_SPEED: f32 = 0.4;
pub const MAX_LIQUID_SPEED: f32 = 30.0;

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
struct XmlFlag;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct XmlColor(Option<GameColor>);

impl Serialize for XmlColor {
    fn serialize<S: SerdeSerializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0.map_or("", color_to_xml))
    }
}

impl<'de> Deserialize<'de> for XmlColor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self(GameColor::from_str_loose(&s)))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct XmlShape(Option<PieceShape>);

impl Serialize for XmlShape {
    fn serialize<S: SerdeSerializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0.map_or("Elbow", shape_to_xml))
    }
}

impl<'de> Deserialize<'de> for XmlShape {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self(PieceShape::from_str_loose(&s)))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct XmlPieceType(Option<PieceType>);

impl Serialize for XmlPieceType {
    fn serialize<S: SerdeSerializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0.map_or("Pipe", piece_type_to_xml))
    }
}

impl<'de> Deserialize<'de> for XmlPieceType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self(PieceType::from_str_loose(&s)))
    }
}

/// Single unified XML schema definition for both reading (`<response><game>`,
/// `<reponse><game>`, legacy `<levels>`) and writing canonical Aqualux XML.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename = "response")]
struct XmlResponse {
    #[serde(rename = "@name", default, skip_serializing_if = "Option::is_none")]
    legacy_name: Option<String>,
    #[serde(rename = "@author", default, skip_serializing_if = "Option::is_none")]
    legacy_author: Option<String>,
    #[serde(
        rename = "@description",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    legacy_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    game: Option<XmlGame>,
    #[serde(rename = "level", default, skip_serializing_if = "Vec::is_empty")]
    legacy_levels: Vec<XmlLevel>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct XmlGame {
    #[serde(rename = "@id", default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(rename = "@title", default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(rename = "@name", default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(rename = "@owner", default, skip_serializing_if = "Option::is_none")]
    owner: Option<String>,
    #[serde(rename = "@author", default, skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    #[serde(
        rename = "@description",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    description: Option<String>,
    #[serde(rename = "level", default)]
    levels: Vec<XmlLevel>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct XmlLevel {
    #[serde(rename = "@index", default, skip_serializing_if = "Option::is_none")]
    index: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(rename = "levelData", default, skip_serializing_if = "Option::is_none")]
    level_data: Option<XmlLevelData>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct XmlLevelData {
    #[serde(
        rename = "backgroundId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    background_id: Option<String>,
    #[serde(
        rename = "liquidColor",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    liquid_color: Option<XmlColor>,
    #[serde(
        rename = "startDelay",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    start_delay: Option<String>,
    #[serde(
        rename = "liquidSpeed",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    liquid_speed: Option<String>,
    #[serde(
        rename = "rotaterColor",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    rotater_color: Option<XmlColor>,
    #[serde(
        rename = "moverColor",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    mover_color: Option<XmlColor>,
    #[serde(
        rename = "stationaryColor",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    stationary_color: Option<XmlColor>,
    #[serde(rename = "gateColor", default, skip_serializing_if = "Option::is_none")]
    gate_color: Option<XmlColor>,
    #[serde(
        rename = "freezeColor",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    freeze_color: Option<XmlColor>,
    #[serde(
        rename = "magicColor",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    magic_color: Option<XmlColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    par: Option<XmlPar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tutorial: Option<XmlTutorial>,
    #[serde(rename = "tileGrid", default, skip_serializing_if = "Option::is_none")]
    tile_grid: Option<XmlTileGrid>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct XmlPar {
    #[serde(rename = "@moves", default, skip_serializing_if = "Option::is_none")]
    moves: Option<String>,
    #[serde(rename = "@pipes", default, skip_serializing_if = "Option::is_none")]
    pipes: Option<String>,
    #[serde(rename = "@time", default, skip_serializing_if = "Option::is_none")]
    time: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct XmlTutorial {
    #[serde(default)]
    message: Vec<XmlTutorialMsg>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct XmlTutorialMsg {
    #[serde(rename = "@tile", default, skip_serializing_if = "Option::is_none")]
    tile: Option<String>,
    #[serde(
        rename = "@waitForSelect",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    wait_for_select: Option<String>,
    #[serde(
        rename = "@waitForSwap",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    wait_for_swap: Option<String>,
    #[serde(
        rename = "@waitForDrag",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    wait_for_drag: Option<String>,
    #[serde(
        rename = "@holdHilight",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    hold_hilight: Option<String>,
    #[serde(rename = "$text", default)]
    text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct XmlTileGrid {
    #[serde(rename = "@cols", default, skip_serializing_if = "Option::is_none")]
    cols: Option<String>,
    #[serde(rename = "@rows", default, skip_serializing_if = "Option::is_none")]
    rows: Option<String>,
    #[serde(default)]
    tile: Vec<XmlTile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct XmlTile {
    #[serde(rename = "@x", default, skip_serializing_if = "Option::is_none")]
    x: Option<String>,
    #[serde(rename = "@y", default, skip_serializing_if = "Option::is_none")]
    y: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shape: Option<XmlShape>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    piece_type: Option<XmlPieceType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    direction: Option<String>,
    #[serde(
        rename = "isMoveable",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    is_moveable: Option<XmlFlag>,
    #[serde(rename = "canRotate", default, skip_serializing_if = "Option::is_none")]
    can_rotate: Option<XmlFlag>,
    #[serde(rename = "isGate", default, skip_serializing_if = "Option::is_none")]
    is_gate: Option<XmlFlag>,
    #[serde(rename = "isFreeze", default, skip_serializing_if = "Option::is_none")]
    is_freeze: Option<XmlFlag>,
    #[serde(rename = "isMagic", default, skip_serializing_if = "Option::is_none")]
    is_magic: Option<XmlFlag>,
    #[serde(
        rename = "secondaryColor",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    secondary_color: Option<XmlColor>,
}

impl XmlResponse {
    fn from_pack(pack: &CustomPackFile) -> Self {
        let levels = pack
            .levels
            .iter()
            .enumerate()
            .map(|(idx, lvl)| XmlLevel::from_level_def(idx, lvl))
            .collect();
        Self {
            legacy_name: None,
            legacy_author: None,
            legacy_description: None,
            game: Some(XmlGame {
                id: Some("mmg.aqualux".to_string()),
                title: Some(pack.meta.name.clone()),
                name: None,
                owner: Some(pack.meta.author.clone()),
                author: None,
                description: Some(pack.meta.description.clone()),
                levels,
            }),
            legacy_levels: Vec::new(),
        }
    }

    fn into_levels_and_meta(self) -> (CustomPackMeta, Vec<LevelDef>) {
        let mut meta = CustomPackMeta::default();
        let raw_levels = if let Some(game) = self.game {
            if let Some(n) = game
                .title
                .as_deref()
                .or(game.name.as_deref())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                meta.name = n.to_string();
            }
            if let Some(a) = game
                .owner
                .as_deref()
                .or(game.author.as_deref())
                .map(str::trim)
            {
                meta.author = a.to_string();
            }
            if let Some(d) = game.description.as_deref().map(str::trim) {
                meta.description = d.to_string();
            }
            game.levels
        } else {
            if let Some(n) = self
                .legacy_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                meta.name = n.to_string();
            }
            if let Some(a) = self.legacy_author.as_deref().map(str::trim) {
                meta.author = a.to_string();
            }
            if let Some(d) = self.legacy_description.as_deref().map(str::trim) {
                meta.description = d.to_string();
            }
            self.legacy_levels
        };

        let mut levels: Vec<LevelDef> = raw_levels
            .into_iter()
            .enumerate()
            .filter_map(|(idx, lvl)| lvl.into_level_def(idx))
            .collect();
        levels.sort_by_key(|l| l.index);
        (meta, levels)
    }
}

impl XmlLevel {
    fn from_level_def(idx: usize, lvl: &LevelDef) -> Self {
        let start_delay = lvl.start_delay.round().clamp(0.0, MAX_START_DELAY) as u32;
        let liquid_speed = lvl.liquid_speed.clamp(MIN_LIQUID_SPEED, MAX_LIQUID_SPEED);
        let tutorial = (!lvl.tutorials.is_empty()).then(|| XmlTutorial {
            message: lvl
                .tutorials
                .iter()
                .map(|tut| XmlTutorialMsg {
                    tile: (!tut.tiles.is_empty()).then(|| format_point_list(&tut.tiles)),
                    wait_for_select: tut.wait_for_select.map(|(x, y)| format!("{},{}", x, y)),
                    wait_for_swap: (!tut.wait_for_swap.is_empty())
                        .then(|| format_point_list(&tut.wait_for_swap)),
                    wait_for_drag: (!tut.wait_for_drag.is_empty())
                        .then(|| format_point_list(&tut.wait_for_drag)),
                    hold_hilight: tut.hold_hilight.then(|| "true".to_string()),
                    text: tut.text.clone(),
                })
                .collect(),
        });
        let tiles = lvl
            .tiles
            .iter()
            .map(|t| XmlTile {
                x: Some(t.x.to_string()),
                y: Some(t.y.to_string()),
                shape: Some(XmlShape(Some(t.shape))),
                piece_type: Some(XmlPieceType(Some(t.piece_type))),
                direction: Some((t.direction % 4).to_string()),
                is_moveable: t.is_moveable.then_some(XmlFlag),
                can_rotate: t.can_rotate.then_some(XmlFlag),
                is_gate: t.is_gate.then_some(XmlFlag),
                is_freeze: t.is_freeze.then_some(XmlFlag),
                is_magic: t.is_magic.then_some(XmlFlag),
                secondary_color: t.secondary_color.map(|c| XmlColor(Some(c))),
            })
            .collect();

        Self {
            index: Some(idx.to_string()),
            title: Some(lvl.title.clone()),
            description: Some(lvl.description.clone()),
            level_data: Some(XmlLevelData {
                background_id: Some(lvl.background_id.clone()),
                liquid_color: Some(XmlColor(Some(lvl.liquid_color))),
                start_delay: Some(start_delay.to_string()),
                liquid_speed: Some(liquid_speed.to_string()),
                rotater_color: Some(XmlColor(Some(lvl.rotater_color))),
                mover_color: Some(XmlColor(Some(lvl.mover_color))),
                stationary_color: Some(XmlColor(Some(lvl.stationary_color))),
                gate_color: Some(XmlColor(Some(lvl.gate_color))),
                freeze_color: Some(XmlColor(Some(lvl.freeze_color))),
                magic_color: Some(XmlColor(Some(lvl.magic_color))),
                par: Some(XmlPar {
                    moves: Some(lvl.par_moves.to_string()),
                    pipes: Some(lvl.par_pipes.to_string()),
                    time: Some(lvl.par_time_ms.to_string()),
                }),
                tutorial,
                tile_grid: Some(XmlTileGrid {
                    cols: Some(lvl.cols.to_string()),
                    rows: Some(lvl.rows.to_string()),
                    tile: tiles,
                }),
            }),
        }
    }

    fn into_level_def(self, fallback_idx: usize) -> Option<LevelDef> {
        let index = self
            .index
            .as_deref()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(fallback_idx);
        let title = self
            .title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("Level {}", index + 1));
        let description = self
            .description
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .to_string();

        let ld = self.level_data?;
        let grid = ld.tile_grid?;

        let background_id = normalize_bg_id(ld.background_id.as_deref().unwrap_or("NightHills"));
        let liquid_color = ld.liquid_color.and_then(|c| c.0).unwrap_or(GameColor::Blue);
        let start_delay = ld
            .start_delay
            .as_deref()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .unwrap_or(10.0);
        let liquid_speed = ld
            .liquid_speed
            .as_deref()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .unwrap_or(1.5);

        let rotater_color = ld
            .rotater_color
            .and_then(|c| c.0)
            .unwrap_or(GameColor::White);
        let mover_color = ld
            .mover_color
            .and_then(|c| c.0)
            .unwrap_or(GameColor::Yellow);
        let stationary_color = ld
            .stationary_color
            .and_then(|c| c.0)
            .unwrap_or(GameColor::Orange);
        let gate_color = ld.gate_color.and_then(|c| c.0).unwrap_or(GameColor::Violet);
        let freeze_color = ld
            .freeze_color
            .and_then(|c| c.0)
            .unwrap_or(GameColor::Yellow);
        let magic_color = ld.magic_color.and_then(|c| c.0).unwrap_or(GameColor::Green);

        let par_moves = ld
            .par
            .as_ref()
            .and_then(|p| p.moves.as_deref())
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(10);
        let par_pipes = ld
            .par
            .as_ref()
            .and_then(|p| p.pipes.as_deref())
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(5);
        let par_time_ms = ld
            .par
            .as_ref()
            .and_then(|p| p.time.as_deref())
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(10000);

        let mut tutorials = Vec::new();
        if let Some(tut) = ld.tutorial {
            let mut hold_hilight = false;
            for msg in tut.message {
                if msg.hold_hilight.as_deref().map(str::trim) == Some("true") {
                    hold_hilight = true;
                }
                tutorials.push(TutorialMsg {
                    tiles: parse_point_list(msg.tile.as_deref()),
                    wait_for_select: parse_point_list(msg.wait_for_select.as_deref())
                        .into_iter()
                        .next(),
                    wait_for_swap: parse_point_list(msg.wait_for_swap.as_deref()),
                    wait_for_drag: parse_point_list(msg.wait_for_drag.as_deref()),
                    hold_hilight,
                    text: normalize_tutorial_html(&msg.text),
                });
            }
        }

        let cols = grid
            .cols
            .as_deref()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(5);
        let rows = grid
            .rows
            .as_deref()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(5);

        let tiles = grid
            .tile
            .into_iter()
            .map(|t| {
                let piece_type = t.piece_type.and_then(|p| p.0).unwrap_or(PieceType::Pipe);
                TileDef {
                    x: t.x
                        .as_deref()
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(0),
                    y: t.y
                        .as_deref()
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(0),
                    shape: t.shape.and_then(|s| s.0).unwrap_or(PieceShape::Elbow),
                    piece_type,
                    direction: t
                        .direction
                        .as_deref()
                        .and_then(|s| s.trim().parse::<u8>().ok())
                        .unwrap_or(0)
                        % 4,
                    is_moveable: t.is_moveable.is_some(),
                    can_rotate: t.can_rotate.is_some(),
                    is_gate: t.is_gate.is_some(),
                    is_freeze: t.is_freeze.is_some(),
                    is_magic: t.is_magic.is_some(),
                    secondary_color: t.secondary_color.and_then(|c| c.0),
                }
            })
            .collect();

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
}

fn parse_xml_response(xml_raw: &str) -> Result<XmlResponse, String> {
    let trimmed = xml_raw.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() || !trimmed.starts_with('<') {
        return Err("XML parse error: empty or non-XML input".to_string());
    }
    xml_from_str::<XmlResponse>(trimmed).map_err(|e| format!("XML parse error: {}", e))
}

pub fn parse_levels_xml(xml_raw: &str) -> Result<Vec<LevelDef>, String> {
    let (_, levels) = parse_xml_response(xml_raw)?.into_levels_and_meta();
    Ok(levels)
}

pub fn parse_pack_xml(xml_raw: &str, fallback_name: &str) -> Result<CustomPackFile, String> {
    if xml_raw.len() > MAX_CUSTOM_XML_BYTES {
        return Err(format!(
            "XML exceeds {} byte limit ({} bytes)",
            MAX_CUSTOM_XML_BYTES,
            xml_raw.len()
        ));
    }

    let (mut meta, mut levels) = parse_xml_response(xml_raw)?.into_levels_and_meta();
    if meta.name.is_empty() {
        let fb = fallback_name.trim();
        meta.name = if fb.is_empty() {
            "Custom Pack".to_string()
        } else {
            fb.to_string()
        };
    }

    if levels.is_empty() {
        return Err("XML file contains no levels".to_string());
    }

    for (i, lvl) in levels.iter_mut().enumerate() {
        lvl.index = i;
        lvl.cols = lvl.cols.clamp(MIN_GRID_DIM, MAX_GRID_COLS);
        lvl.rows = lvl.rows.clamp(MIN_GRID_DIM, MAX_GRID_ROWS);
        lvl.start_delay = if lvl.start_delay.is_finite() {
            lvl.start_delay.round().clamp(0.0, MAX_START_DELAY)
        } else {
            10.0
        };
        lvl.liquid_speed = if lvl.liquid_speed.is_finite() {
            lvl.liquid_speed.clamp(MIN_LIQUID_SPEED, MAX_LIQUID_SPEED)
        } else {
            1.5
        };
        lvl.tiles.retain(|t| t.x < lvl.cols && t.y < lvl.rows);
    }

    Ok(CustomPackFile { meta, levels })
}

pub fn write_pack_xml(pack: &CustomPackFile) -> String {
    let response = XmlResponse::from_pack(pack);
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let mut ser = Serializer::new(&mut out);
    ser.indent(' ', 2);
    response
        .serialize(ser)
        .expect("serializing valid LevelDef pack to XML cannot fail");
    out.push('\n');
    out
}

fn shape_to_xml(s: PieceShape) -> &'static str {
    match s {
        PieceShape::Elbow => "Elbow",
        PieceShape::Straight => "Straight",
        PieceShape::Cross => "Cross",
        PieceShape::Spigot | PieceShape::Drain => "Spigot",
        PieceShape::Block => "Block",
    }
}

fn piece_type_to_xml(p: PieceType) -> &'static str {
    match p {
        PieceType::Source => "Source",
        PieceType::Pipe => "Pipe",
        PieceType::Drain => "Drain",
        PieceType::Blocker => "Blocker",
        PieceType::Portal => "Portal",
        PieceType::FrozenPipe => "FrozenPipe",
    }
}

fn color_to_xml(c: GameColor) -> &'static str {
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
                    shape: PieceShape::Spigot,
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

fn format_point_list(pts: &[(usize, usize)]) -> String {
    pts.iter()
        .map(|(x, y)| format!("{},{}", x, y))
        .collect::<Vec<_>>()
        .join("|")
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

fn canonical_preserved_tag(raw_tag: &str) -> Option<String> {
    let inner = raw_tag.trim();
    if inner.eq_ignore_ascii_case("i") {
        return Some("<i>".to_string());
    }
    if inner.eq_ignore_ascii_case("/i") {
        return Some("</i>".to_string());
    }
    if inner.eq_ignore_ascii_case("/font") {
        return Some("</font>".to_string());
    }
    let mut rest = inner;
    if !(rest.len() >= 4 && rest[..4].eq_ignore_ascii_case("font")) {
        return None;
    }
    rest = &rest[4..];
    if !rest.starts_with(|c: char| c.is_ascii_whitespace()) {
        return None;
    }
    let lower = rest.to_ascii_lowercase();
    let idx = lower.find("color=")?;
    let val = rest[idx + 6..].trim_start();
    let raw_hex = if let Some(s) = val.strip_prefix('"') {
        s.split('"').next()?
    } else if let Some(s) = val.strip_prefix('\'') {
        s.split('\'').next()?
    } else {
        val.split(|c: char| c.is_ascii_whitespace() || c == '>')
            .next()?
    };
    let hex = raw_hex.trim().strip_prefix('#').unwrap_or(raw_hex.trim());
    if hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(format!("<font color=\"#{}\">", hex.to_ascii_uppercase()))
    } else {
        None
    }
}

fn normalize_tutorial_html(input: &str) -> String {
    let with_breaks = input
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<BR>", "\n")
        .replace("<BR/>", "\n")
        .replace("<BR />", "\n")
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut lines = Vec::new();
    for raw_line in with_breaks.split('\n') {
        let mut line_out = String::with_capacity(raw_line.len());
        let mut pending_space = false;
        let mut chars = raw_line.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '<' {
                let mut tag = String::new();
                let mut closed = false;
                for tc in chars.by_ref() {
                    if tc == '>' {
                        closed = true;
                        break;
                    }
                    tag.push(tc);
                }
                if closed && let Some(kept) = canonical_preserved_tag(&tag) {
                    if pending_space && !line_out.is_empty() {
                        line_out.push(' ');
                    }
                    pending_space = false;
                    line_out.push_str(&kept);
                }
            } else if ch == ' ' || ch == '\t' {
                if !line_out.is_empty() {
                    pending_space = true;
                }
            } else {
                if pending_space && !line_out.is_empty() {
                    line_out.push(' ');
                }
                pending_space = false;
                line_out.push(ch);
            }
        }
        lines.push(line_out);
    }
    lines.join("\n").trim_matches('\n').to_string()
}
