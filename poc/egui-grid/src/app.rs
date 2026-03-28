// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Proof-of-concept: tracker pattern grid rendered with egui.
//!
//! Validates that egui can render a pixel-art style fixed-width grid with
//! per-element colouring — the core requirement for the SoymilkyTracker UI.

use egui::{Color32, FontFamily, FontId, RichText, Stroke, Vec2};

// ── Palette ─────────────────────────────────────────────────────────────────
const BG:           Color32 = Color32::from_rgb(0x18, 0x18, 0x18);
const BG_BEAT:      Color32 = Color32::from_rgb(0x22, 0x22, 0x28);
const BG_SELECTED:  Color32 = Color32::from_rgb(0x1a, 0x3a, 0x1a);
const COL_ROW_NUM:  Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
const COL_NOTE:     Color32 = Color32::from_rgb(0xe8, 0xe8, 0xe8);
const COL_EMPTY:    Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
const COL_INST:     Color32 = Color32::from_rgb(0xe8, 0xc4, 0x6a);
const COL_VOL:      Color32 = Color32::from_rgb(0x6a, 0xd4, 0xe8);
const COL_FX:       Color32 = Color32::from_rgb(0xc4, 0x6a, 0xe8);
const COL_FXPARAM:  Color32 = Color32::from_rgb(0xa0, 0x50, 0xc8);
const COL_CURSOR:   Color32 = Color32::from_rgb(0x40, 0xff, 0x40);

// ── Demo data ────────────────────────────────────────────────────────────────
const CHANNELS: usize = 6;
const ROWS:     usize = 32;

#[derive(Clone)]
struct Cell {
    note:     Option<&'static str>, // e.g. "C-4"
    inst:     Option<u8>,
    vol:      Option<u8>,
    fx:       Option<char>,
    fx_param: Option<u8>,
}

impl Cell {
    const EMPTY: Self = Self { note: None, inst: None, vol: None, fx: None, fx_param: None };

    fn note(note: &'static str, inst: u8, vol: u8) -> Self {
        Self { note: Some(note), inst: Some(inst), vol: Some(vol), fx: None, fx_param: None }
    }

    fn note_fx(note: &'static str, inst: u8, vol: u8, fx: char, fx_param: u8) -> Self {
        Self { note: Some(note), inst: Some(inst), vol: Some(vol), fx: Some(fx), fx_param: Some(fx_param) }
    }
}

fn demo_pattern() -> Vec<Vec<Cell>> {
    let mut rows = vec![vec![Cell::EMPTY; CHANNELS]; ROWS];
    // Ch0 — bass kick pattern
    rows[0][0]  = Cell::note("C-2", 1, 0x40);
    rows[4][0]  = Cell::note("C-2", 1, 0x38);
    rows[8][0]  = Cell::note("C-2", 1, 0x40);
    rows[12][0] = Cell::note("C-2", 1, 0x3c);
    rows[16][0] = Cell::note("C-2", 1, 0x40);
    rows[20][0] = Cell::note("C-2", 1, 0x38);
    rows[24][0] = Cell::note("C-2", 1, 0x40);
    rows[28][0] = Cell::note("C-2", 1, 0x3c);
    // Ch1 — hi-hat
    for r in (0..ROWS).step_by(2) {
        rows[r][1] = Cell::note("F#3", 3, 0x20);
    }
    // Ch2 — melody
    rows[0][2]  = Cell::note_fx("E-4", 2, 0x50, 'V', 0x08);
    rows[4][2]  = Cell::note("G-4", 2, 0x48);
    rows[6][2]  = Cell::note("A-4", 2, 0x44);
    rows[8][2]  = Cell::note_fx("B-4", 2, 0x50, 'V', 0x06);
    rows[12][2] = Cell::note("G-4", 2, 0x40);
    rows[16][2] = Cell::note_fx("E-4", 2, 0x4c, 'Q', 0x03);
    rows[24][2] = Cell::note("D-4", 2, 0x48);
    // Ch3 — chords
    rows[0][3]  = Cell::note("C-3", 4, 0x30);
    rows[8][3]  = Cell::note("A-2", 4, 0x30);
    rows[16][3] = Cell::note("F-2", 4, 0x30);
    rows[24][3] = Cell::note("G-2", 4, 0x30);
    rows
}

// ── App ──────────────────────────────────────────────────────────────────────
pub struct TrackerGridApp {
    pattern:      Vec<Vec<Cell>>,
    selected_row: usize,
    selected_ch:  usize,
}

impl TrackerGridApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            pattern:      demo_pattern(),
            selected_row: 0,
            selected_ch:  0,
        }
    }
}

impl eframe::App for TrackerGridApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Dark background
        ctx.set_visuals(egui::Visuals::dark());

        // Handle arrow key navigation
        ctx.input(|i| {
            if i.key_pressed(egui::Key::ArrowDown)  { self.selected_row = (self.selected_row + 1).min(ROWS - 1); }
            if i.key_pressed(egui::Key::ArrowUp)    { self.selected_row = self.selected_row.saturating_sub(1); }
            if i.key_pressed(egui::Key::ArrowRight) { self.selected_ch  = (self.selected_ch  + 1).min(CHANNELS - 1); }
            if i.key_pressed(egui::Key::ArrowLeft)  { self.selected_ch  = self.selected_ch.saturating_sub(1); }
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                let font = FontId::new(13.0, FontFamily::Monospace);

                // ── Header ──────────────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.add_space(36.0); // row number gutter
                    for ch in 0..CHANNELS {
                        let label = RichText::new(format!(" Ch{:<2}              ", ch + 1))
                            .font(font.clone())
                            .color(COL_INST);
                        ui.label(label);
                    }
                });
                ui.add(egui::Separator::default().horizontal().spacing(2.0));

                // ── Pattern rows ─────────────────────────────────────────────
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 1.0);

                    for row in 0..ROWS {
                        let is_beat    = row % 4 == 0;
                        let is_sel_row = row == self.selected_row;
                        let row_bg     = if is_sel_row { BG_SELECTED }
                                         else if is_beat { BG_BEAT }
                                         else { BG };

                        let resp = ui.horizontal(|ui| {
                            // Row number
                            let row_str = RichText::new(format!("{:02X} ", row))
                                .font(font.clone())
                                .color(if is_beat { COL_CURSOR } else { COL_ROW_NUM });
                            ui.label(row_str);

                            // Cells
                            for ch in 0..CHANNELS {
                                let is_sel_cell = is_sel_row && ch == self.selected_ch;
                                render_cell(ui, &self.pattern[row][ch], &font, is_sel_cell);

                                if ch < CHANNELS - 1 {
                                    ui.add(egui::Separator::default().vertical().spacing(2.0));
                                }
                            }
                        });

                        // Row background
                        let rect = resp.response.rect;
                        ui.painter().rect_filled(rect, 0.0, row_bg);
                        if is_sel_row {
                            ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, COL_CURSOR));
                        }
                    }
                });
            });
    }
}

fn render_cell(ui: &mut egui::Ui, cell: &Cell, font: &FontId, selected: bool) {
    let note_col = if selected { COL_CURSOR } else { COL_NOTE };

    // Note (3 chars)
    let note_text = match cell.note {
        Some(n) => RichText::new(format!("{} ", n)).font(font.clone()).color(note_col),
        None    => RichText::new("--- ").font(font.clone()).color(COL_EMPTY),
    };
    ui.label(note_text);

    // Instrument (2 hex chars)
    let inst_text = match cell.inst {
        Some(i) => RichText::new(format!("{:02X} ", i)).font(font.clone()).color(COL_INST),
        None    => RichText::new("-- ").font(font.clone()).color(COL_EMPTY),
    };
    ui.label(inst_text);

    // Volume (2 hex chars)
    let vol_text = match cell.vol {
        Some(v) => RichText::new(format!("{:02X} ", v)).font(font.clone()).color(COL_VOL),
        None    => RichText::new(".. ").font(font.clone()).color(COL_EMPTY),
    };
    ui.label(vol_text);

    // Effect (1 char)
    let fx_text = match cell.fx {
        Some(f) => RichText::new(format!("{}", f)).font(font.clone()).color(COL_FX),
        None    => RichText::new("-").font(font.clone()).color(COL_EMPTY),
    };
    ui.label(fx_text);

    // Effect param (2 hex chars)
    let fxp_text = match cell.fx_param {
        Some(p) => RichText::new(format!("{:02X} ", p)).font(font.clone()).color(COL_FXPARAM),
        None    => RichText::new("-- ").font(font.clone()).color(COL_EMPTY),
    };
    ui.label(fxp_text);
}
