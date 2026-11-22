// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Difficulty {
    Easy,
    #[default]
    Medium,
    Hard,
}

impl Difficulty {
    /// Display name for each difficulty level.
    pub fn human_label(self) -> &'static str {
        match self {
            Self::Easy => "relaxed (easy)",
            Self::Medium => "casual (medium)",
            Self::Hard => "hardcore (hard)",
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub enum Trophy {
    #[default]
    None = 0,
    Bronze = 1,
    Silver = 2,
    Gold = 3,
    Platinum = 4,
}

impl Trophy {
    pub fn badge_label(self) -> &'static str {
        match self {
            Self::None => "Try again for a Trophy",
            Self::Bronze => "Bronze Trophy",
            Self::Silver => "Silver Trophy",
            Self::Gold => "Gold Trophy",
            Self::Platinum => "Platinum Trophy",
        }
    }

    pub fn status_label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Bronze => "Bronze",
            Self::Silver => "Silver",
            Self::Gold => "Gold",
            Self::Platinum => "Platinum",
        }
    }

    pub fn status_color_rgb(self) -> (u8, u8, u8) {
        match self {
            Self::None => (255, 255, 255),
            Self::Bronze => (128, 128, 64),
            Self::Silver => (192, 192, 192),
            Self::Gold => (255, 255, 0),
            Self::Platinum => (0, 168, 249),
        }
    }

    pub fn bitmap_name(self) -> &'static str {
        match self {
            Self::None => "NoTrophyBitmap",
            Self::Bronze => "BronzeTrophyBitmap",
            Self::Silver => "SilverTrophyBitmap",
            Self::Gold => "GoldTrophyBitmap",
            Self::Platinum => "PlatinumTrophyBitmap",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AwardKind {
    LightWater,
    Champ,
    MirrorMaster,
    Ultralux,
    Mystery,
}

impl AwardKind {
    pub fn from_slot(slot: usize, unlocked: bool) -> Self {
        if !unlocked {
            return Self::Mystery;
        }
        match slot {
            0 => Self::LightWater,
            1 => Self::Champ,
            2 => Self::MirrorMaster,
            _ => Self::Ultralux,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Mystery => "??????????",
            Self::LightWater => "LIGHT WATER",
            Self::MirrorMaster => "MIRROR ZEN",
            Self::Ultralux => "ULTRALUXER",
            Self::Champ => "LUX CHAMP",
        }
    }

    pub fn bitmap_name(self) -> &'static str {
        match self {
            Self::Mystery => "MysteryTrophyBitmap",
            Self::MirrorMaster => "MirrorTrophyBitmap",
            Self::Ultralux => "UltraluxTrophyBitmap",
            Self::LightWater => "LightTrophyBitmap",
            Self::Champ => "ChampTrophyBitmap",
        }
    }

    pub fn notification(self) -> &'static str {
        match self {
            Self::MirrorMaster => "for beating 50 mirror matches!",
            Self::Ultralux => "for getting 100 platinum trophies!",
            Self::LightWater => "for beating 25 story levels!",
            Self::Champ => "for finishing story mode!",
            Self::Mystery => "",
        }
    }

    pub fn descrip1(self) -> &'static str {
        match self {
            Self::Mystery => " ???????????????",
            Self::MirrorMaster => "  for beating 50",
            Self::Ultralux => "  for getting 100",
            Self::LightWater => "  for beating 25",
            Self::Champ => "  for finishing the",
        }
    }

    pub fn descrip2(self) -> &'static str {
        match self {
            Self::Mystery => " ???????????????",
            Self::MirrorMaster => "  mirror matches",
            Self::Ultralux => "platinum trophies",
            Self::LightWater => "    story levels",
            Self::Champ => "    story mode",
        }
    }
}
