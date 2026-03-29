// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Main application struct and egui update loop.

use tracker_engine::xm::{XmCell, XmNote, XmPattern};

use crate::pattern_editor::PatternEditor;

/// IBM EGA 8×8 bitmap font (Ac437 variant, CC BY 4.0, VileR / int10h.org).
/// Used as the primary UI typeface for the classic DOS tracker aesthetic.
const IBM_EGA_8X8: &[u8] = include_bytes!("../../../assets/fonts/Ac437_IBM_EGA_8x8.ttf");

/// Name under which the tracker font is registered in egui's font system.
pub const FONT_TRACKER: &str = "tracker";

/// Install the IBM EGA 8×8 font into an egui context.
///
/// Call this once during app setup via [`eframe::CreationContext`].
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        FONT_TRACKER.to_owned(),
        egui::FontData::from_static(IBM_EGA_8X8),
    );

    // Make the tracker font the first choice for both Proportional and Monospace
    // families so it is used by default throughout the UI.
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, FONT_TRACKER.to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, FONT_TRACKER.to_owned());
    // Register the named family so FontFamily::Name("tracker") resolves.
    fonts
        .families
        .entry(egui::FontFamily::Name(FONT_TRACKER.into()))
        .or_default()
        .push(FONT_TRACKER.to_owned());

    ctx.set_fonts(fonts);
}

// ── TrackerApp ────────────────────────────────────────────────────────────────

pub struct TrackerApp {
    pattern: XmPattern,
    editor: PatternEditor,
}

impl TrackerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        Self {
            pattern: demo_pattern(),
            editor: PatternEditor::new(),
        }
    }
}

impl eframe::App for TrackerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Status bar at the top
        egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("SoymilkyTracker");
                ui.separator();
                ui.label(format!(
                    "Row {:02X}  Ch {}  {}  Oct {}  Stp {}",
                    self.editor.cursor_row,
                    self.editor.cursor_channel + 1,
                    subcol_label(self.editor.cursor_col),
                    self.editor.octave,
                    self.editor.step,
                ));
            });
        });

        // Pattern editor fills the remaining central area
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                self.editor.show(ui, &mut self.pattern);
            });
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn subcol_label(col: crate::pattern_editor::SubCol) -> &'static str {
    use crate::pattern_editor::SubCol;
    match col {
        SubCol::Note => "Note",
        SubCol::InsHi | SubCol::InsLo => "Inst",
        SubCol::VolHi | SubCol::VolLo => "Vol ",
        SubCol::FxLtr => "FxLt",
        SubCol::OpHi | SubCol::OpLo => "FxOp",
    }
}

/// Build a 64-row × 4-channel demo pattern with representative note data.
fn demo_pattern() -> XmPattern {
    let channels = 4;
    let rows = 64;
    let mut grid: Vec<Vec<XmCell>> = vec![vec![XmCell::default(); channels]; rows];

    // Channel 0 — kick-like: C-4 every 4 rows, set-volume column
    for r in (0..rows).step_by(4) {
        grid[r][0] = XmCell {
            note: XmNote::On(49), // C-4
            instrument: 1,
            volume: 0x50, // set volume 64
            effect: 0,
            effect_param: 0,
        };
    }

    // Channel 1 — bass: alternating G-3 / G-4
    let bass_notes = [
        (8, 44u8),
        (16, 56),
        (24, 44),
        (32, 56),
        (40, 44),
        (48, 56),
        (56, 44),
    ];
    for (r, note) in bass_notes {
        grid[r][1] = XmCell {
            note: XmNote::On(note),
            instrument: 2,
            volume: 0x40,
            effect: 0,
            effect_param: 0,
        };
    }

    // Channel 2 — melody with effects
    let melody: &[(usize, u8, u8, u8)] = &[
        (0, 53, 0x04, 0x28),  // E-4 vibrato
        (4, 55, 0x0A, 0x04),  // F#4 vol-slide up
        (8, 57, 0, 0),        // A-4
        (12, 55, 0, 0),       // F#4
        (16, 53, 0x04, 0x18), // E-4 vibrato
        (20, 52, 0, 0),       // E-4 (Eb)
        (24, 50, 0, 0),       // D-4
        (28, 48, 0, 0),       // C#4
        (32, 49, 0, 0),       // C-4
        (48, 53, 0x0C, 0x40), // E-4 set-vol C40
        (56, 57, 0x0C, 0x30), // A-4 set-vol C30
    ];
    for &(r, note, fx, fp) in melody {
        grid[r][2] = XmCell {
            note: XmNote::On(note),
            instrument: 3,
            volume: 0,
            effect: fx,
            effect_param: fp,
        };
    }

    // Channel 3 — hi-hat with note-off
    for r in (2..rows).step_by(4) {
        grid[r][3] = XmCell {
            note: XmNote::On(61), // C-5
            instrument: 4,
            volume: 0x30,
            effect: 0,
            effect_param: 0,
        };
    }
    // Occasional key-off
    for r in [10, 26, 42, 58] {
        grid[r][3] = XmCell {
            note: XmNote::Off,
            instrument: 0,
            volume: 0,
            effect: 0,
            effect_param: 0,
        };
    }

    XmPattern { rows: grid }
}
