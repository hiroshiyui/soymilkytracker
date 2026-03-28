// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! In-memory representation of a tracker composition.
//! Serializable for save/load; format-agnostic (XM/MOD are parsed into this).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Composition {
    pub title: String,
    pub bpm: u16,
    pub speed: u8,
    pub channels: u8,
    pub instruments: Vec<Instrument>,
    pub patterns: Vec<Pattern>,
    /// Order list: sequence of pattern indices for playback.
    pub order: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instrument {
    pub name: String,
    // Sample data and loop points will be expanded in Phase 1.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub rows: Vec<Vec<Cell>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cell {
    pub note: Option<u8>,
    pub instrument: Option<u8>,
    pub volume: Option<u8>,
    pub effect: Option<u8>,
    pub effect_param: Option<u8>,
}
