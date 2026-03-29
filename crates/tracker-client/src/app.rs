// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Main application struct and egui update loop.
//!
//! [`TrackerApp`] owns the complete [`XmModule`] being edited, the audio
//! controller, and all UI-widget state.  The [`eframe::App::update`] method
//! assembles the multi-panel layout defined in `doc/ui-mockups.md`:
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │  TITLE BAR (song name + status)          │
//! ├─────────────────────────────────────────┤
//! │  CONTROLS  (order · speed · pattern)     │
//! ├─────────────────────────────────────────┤
//! │  MENU BUTTONS (transport + stubs)        │
//! ├─────────────────────────────────────────┤
//! │  INSTRUMENT PANEL (list │ sample list)  │
//! ├─────────────────────────────────────────┤
//! │  PATTERN EDITOR  (central panel)         │
//! └─────────────────────────────────────────┘
//! ```

use std::sync::Arc;

use egui::{Align2, Color32, FontFamily, FontId, Frame, Margin, RichText, Sense, Ui, Vec2};
use tracker_engine::{
    TrackerAudio,
    xm::{XmCell, XmEnvelope, XmInstrument, XmModule, XmNote, XmPattern},
};
#[cfg(not(target_arch = "wasm32"))]
use tracker_engine::backend::NativeAudioBackend;
#[cfg(target_arch = "wasm32")]
use tracker_engine::backend::WasmAudioBackend;

use crate::pattern_editor::{PatternEditor, ROW_H};

// ── Font helpers ──────────────────────────────────────────────────────────────

/// IBM EGA 8×8 bitmap font (Ac437 variant, CC BY 4.0, VileR / int10h.org).
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

/// Convenience shorthand: 8 px tracker-font `FontId`.
fn font8() -> FontId {
    FontId::new(8.0, FontFamily::Name(FONT_TRACKER.into()))
}

// ── Colour palette (Classic MilkyTracker + UI chrome) ────────────────────────

const C_CHROME: Color32 = Color32::from_rgb(64, 96, 128);
const C_CHROME_LIGHT: Color32 = Color32::from_rgb(100, 140, 180);
const C_PANEL_BG: Color32 = Color32::from_rgb(40, 48, 56);
const C_LIST_BG: Color32 = Color32::from_rgb(32, 32, 48);
const C_SELECTED_ROW: Color32 = Color32::from_rgb(64, 64, 128);
const C_INST_IDX: Color32 = Color32::from_rgb(128, 224, 255);
const C_TEXT: Color32 = Color32::WHITE;
const C_MUTED: Color32 = Color32::from_rgb(96, 96, 96);
const C_PLAY_BG: Color32 = Color32::from_rgb(0, 96, 0);
const C_STOP_LIT: Color32 = Color32::from_rgb(80, 24, 24);
const C_BTN_DIM: Color32 = Color32::from_rgb(48, 48, 48);
const C_REC_BG: Color32 = Color32::from_rgb(160, 24, 48);

// ── TrackerApp ────────────────────────────────────────────────────────────────

/// Root application state — owns the module, audio engine, and all UI widget state.
pub struct TrackerApp {
    /// The full XM module currently being composed and edited.
    module: XmModule,
    /// Audio engine controller (wraps the platform backend and the player).
    audio: TrackerAudio,
    /// Pattern editor widget (cursor position, record mode, octave, step).
    editor: PatternEditor,
    /// Index into `module.patterns` currently displayed in the pattern editor.
    current_pattern_idx: usize,
    /// Index into `module.pattern_order` currently selected in the order list.
    current_order_idx: usize,
    /// Selected instrument — 0-based index into `module.instruments`.
    selected_instrument: usize,
    /// Whether record mode is active (note entry while playing).
    record_mode: bool,
}

impl TrackerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        Self {
            module: default_module(),
            audio: make_audio(),
            editor: PatternEditor::new(),
            current_pattern_idx: 0,
            current_order_idx: 0,
            selected_instrument: 0,
            record_mode: false,
        }
    }

    // ── Transport actions ─────────────────────────────────────────────────────

    /// Start song playback from the currently selected order-list position.
    fn play_song(&mut self) {
        let arc = Arc::new(self.module.clone());
        self.audio.load(arc);
        self.audio.seek(self.current_order_idx, 0);
        let _ = self.audio.play();
    }

    /// Loop the currently selected pattern in isolation.
    ///
    /// Builds a single-entry order module so the player loops just this pattern.
    fn play_pattern(&mut self) {
        let mut m = self.module.clone();
        m.song_length = 1;
        m.restart_position = 0;
        m.pattern_order = vec![self.current_pattern_idx as u8];
        self.audio.load(Arc::new(m));
        let _ = self.audio.play();
    }

    /// Stop playback, rewind the player, and close the audio stream.
    fn stop(&mut self) {
        self.audio.stop();
    }

    // ── Panel rendering ───────────────────────────────────────────────────────

    /// Title bar: song name editor + live playback status + cursor info.
    fn show_title_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("SoymilkyTracker").color(C_CHROME_LIGHT));
            ui.separator();

            ui.label(RichText::new("Title:").color(C_CHROME_LIGHT));
            ui.add(egui::TextEdit::singleline(&mut self.module.name).desired_width(160.0));

            ui.separator();

            // Live playback position
            if self.audio.is_playing() {
                if let Some(pos) = self.audio.position() {
                    ui.label(
                        RichText::new(format!(
                            "▶  Ord {:02X}  Row {:02X}  BPM {}",
                            pos.order, pos.row, pos.bpm,
                        ))
                        .color(Color32::from_rgb(128, 255, 128)),
                    );
                }
            } else {
                ui.label(RichText::new("■  Stopped").color(C_MUTED));
            }

            ui.separator();

            // Editor cursor status
            ui.label(
                RichText::new(format!(
                    "Row {:02X}  Ch {}  {}  Oct {}  Stp {}",
                    self.editor.cursor_row,
                    self.editor.cursor_channel + 1,
                    subcol_label(self.editor.cursor_col),
                    self.editor.octave,
                    self.editor.step,
                ))
                .color(C_TEXT),
            );
        });
    }

    /// Controls row: order navigation · BPM/TPB/step/octave · pattern controls.
    fn show_controls_row(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // ── Order section ──────────────────────────────────────────────
            ui.group(|ui| {
                ui.label(RichText::new("ORD").color(C_CHROME_LIGHT));

                let ord_count = self.module.pattern_order.len().max(1);
                let cur_pat = self
                    .module
                    .pattern_order
                    .get(self.current_order_idx)
                    .copied()
                    .unwrap_or(0);

                // Current position display: [order pos] → [pattern index]
                ui.label(
                    RichText::new(format!("{:02X}→{:02X}", self.current_order_idx, cur_pat))
                        .color(C_INST_IDX),
                );

                // Navigate order list
                if ui.small_button("◀").clicked() && self.current_order_idx > 0 {
                    self.current_order_idx -= 1;
                    self.current_pattern_idx =
                        self.module.pattern_order[self.current_order_idx] as usize;
                }
                if ui.small_button("▶").clicked() && self.current_order_idx + 1 < ord_count {
                    self.current_order_idx += 1;
                    self.current_pattern_idx =
                        self.module.pattern_order[self.current_order_idx] as usize;
                }
                ui.label(
                    RichText::new(format!("/{:02X}", ord_count.saturating_sub(1))).color(C_MUTED),
                );

                // Increment / decrement the pattern index at the current order entry
                if ui.small_button("+").clicked() {
                    let n_pats = self.module.patterns.len().saturating_sub(1) as u8;
                    let new_pat = cur_pat.saturating_add(1).min(n_pats);
                    if let Some(e) = self.module.pattern_order.get_mut(self.current_order_idx) {
                        *e = new_pat;
                        self.current_pattern_idx = new_pat as usize;
                    }
                }
                if ui.small_button("−").clicked() {
                    let new_pat = cur_pat.saturating_sub(1);
                    if let Some(e) = self.module.pattern_order.get_mut(self.current_order_idx) {
                        *e = new_pat;
                        self.current_pattern_idx = new_pat as usize;
                    }
                }

                // Insert / delete order entries
                if ui.small_button("Ins").clicked() {
                    let pos = (self.current_order_idx + 1).min(self.module.pattern_order.len());
                    self.module.pattern_order.insert(pos, cur_pat);
                    self.module.song_length = (self.module.song_length + 1).min(255);
                    self.current_order_idx = pos;
                }
                if ui.small_button("Del").clicked() && self.module.pattern_order.len() > 1 {
                    self.module.pattern_order.remove(self.current_order_idx);
                    self.module.song_length = (self.module.song_length - 1).max(1);
                    if self.current_order_idx >= self.module.pattern_order.len() {
                        self.current_order_idx = self.module.pattern_order.len() - 1;
                    }
                    self.current_pattern_idx =
                        self.module.pattern_order[self.current_order_idx] as usize;
                }
            });

            // ── Speed section ───────────────────────────────────────────────
            ui.group(|ui| {
                // BPM (beats per minute)
                ui.label(RichText::new("BPM").color(C_CHROME_LIGHT));
                ui.label(RichText::new(format!("{:3}", self.module.default_bpm)).color(C_TEXT));
                if ui.small_button("+").clicked() {
                    self.module.default_bpm = (self.module.default_bpm + 1).min(255);
                }
                if ui.small_button("−").clicked() {
                    self.module.default_bpm = (self.module.default_bpm - 1).max(1);
                }

                ui.separator();

                // TPB / speed (ticks per beat / row)
                ui.label(RichText::new("TPB").color(C_CHROME_LIGHT));
                ui.label(
                    RichText::new(format!("{:2}", self.module.default_tempo)).color(C_TEXT),
                );
                if ui.small_button("+").clicked() {
                    self.module.default_tempo = (self.module.default_tempo + 1).min(31);
                }
                if ui.small_button("−").clicked() {
                    self.module.default_tempo = (self.module.default_tempo - 1).max(1);
                }

                ui.separator();

                // Cursor step after note entry
                ui.label(RichText::new("Stp").color(C_CHROME_LIGHT));
                ui.label(RichText::new(format!("{:2}", self.editor.step)).color(C_TEXT));
                if ui.small_button("+").clicked() {
                    self.editor.step = (self.editor.step + 1).min(16);
                }
                if ui.small_button("−").clicked() {
                    self.editor.step = self.editor.step.saturating_sub(1);
                }

                ui.separator();

                // QWERTY piano octave
                ui.label(RichText::new("Oct").color(C_CHROME_LIGHT));
                ui.label(RichText::new(format!("{}", self.editor.octave)).color(C_TEXT));
                if ui.small_button("+").clicked() && self.editor.octave < 8 {
                    self.editor.octave += 1;
                }
                if ui.small_button("−").clicked() && self.editor.octave > 0 {
                    self.editor.octave -= 1;
                }
            });

            // ── Pattern section ─────────────────────────────────────────────
            ui.group(|ui| {
                let n_pats = self.module.patterns.len().max(1);

                // Current pattern index
                ui.label(RichText::new("Pat").color(C_CHROME_LIGHT));
                ui.label(
                    RichText::new(format!("{:02X}", self.current_pattern_idx)).color(C_TEXT),
                );
                if ui.small_button("+").clicked() && self.current_pattern_idx + 1 < n_pats {
                    self.current_pattern_idx += 1;
                    if let Some(e) = self.module.pattern_order.get_mut(self.current_order_idx) {
                        *e = self.current_pattern_idx as u8;
                    }
                }
                if ui.small_button("−").clicked() && self.current_pattern_idx > 0 {
                    self.current_pattern_idx -= 1;
                    if let Some(e) = self.module.pattern_order.get_mut(self.current_order_idx) {
                        *e = self.current_pattern_idx as u8;
                    }
                }

                ui.separator();

                // Row count + expand / shrink
                let row_count = self
                    .module
                    .patterns
                    .get(self.current_pattern_idx)
                    .map(|p| p.rows.len())
                    .unwrap_or(64);
                ui.label(RichText::new("Rows").color(C_CHROME_LIGHT));
                ui.label(RichText::new(format!("{:3}", row_count)).color(C_TEXT));
                if ui.small_button("×2").clicked() {
                    if let Some(pat) = self.module.patterns.get_mut(self.current_pattern_idx) {
                        if pat.rows.len() <= 128 {
                            expand_pattern(pat);
                        }
                    }
                }
                if ui.small_button("/2").clicked() {
                    if let Some(pat) = self.module.patterns.get_mut(self.current_pattern_idx) {
                        if pat.rows.len() >= 4 {
                            shrink_pattern(pat);
                        }
                    }
                }
            });
        });
    }

    /// Menu button row: transport controls and stub buttons for future features.
    fn show_menu_buttons(&mut self, ui: &mut Ui) {
        let is_playing = self.audio.is_playing();

        ui.horizontal(|ui| {
            // ── Transport controls ────────────────────────────────────────
            let btn = egui::Button::new("▶ Play Song")
                .fill(if is_playing && !self.record_mode { C_PLAY_BG } else { C_BTN_DIM });
            if ui.add(btn).clicked() {
                self.play_song();
            }

            if ui.button("▷ Play Pat").clicked() {
                self.play_pattern();
            }

            let btn = egui::Button::new("■ Stop")
                .fill(if !is_playing { C_STOP_LIT } else { C_BTN_DIM });
            if ui.add(btn).clicked() {
                self.stop();
            }

            let rec_text = RichText::new("● Rec")
                .color(if self.record_mode { Color32::WHITE } else { Color32::GRAY });
            let btn = egui::Button::new(rec_text)
                .fill(if self.record_mode { C_REC_BG } else { C_BTN_DIM });
            if ui.add(btn).clicked() {
                self.record_mode = !self.record_mode;
                self.editor.record_mode = self.record_mode;
            }

            ui.separator();

            // ── Stub buttons (not yet implemented) ────────────────────────
            for label in ["Load", "Save", "Smp. Ed.", "Ins. Ed.", "Config", "About"] {
                let _ = ui.button(label);
            }
        });
    }

    /// Instrument panel: instrument list (left) and sample list for the selected instrument (right).
    fn show_instrument_panel(&mut self, ui: &mut Ui) {
        let half_w = (ui.available_width() / 2.0 - 4.0).max(80.0);
        let panel_h = ui.available_height();

        ui.horizontal_top(|ui| {
            // ── Left: instrument list ─────────────────────────────────────
            ui.allocate_ui(Vec2::new(half_w, panel_h), |ui| {
                ui.vertical(|ui| {
                    // Header with add / remove buttons
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Instruments").color(C_CHROME_LIGHT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("−").clicked()
                                && !self.module.instruments.is_empty()
                            {
                                let idx = self
                                    .selected_instrument
                                    .min(self.module.instruments.len() - 1);
                                self.module.instruments.remove(idx);
                                if self.selected_instrument >= self.module.instruments.len()
                                    && !self.module.instruments.is_empty()
                                {
                                    self.selected_instrument =
                                        self.module.instruments.len() - 1;
                                }
                            }
                            if ui.small_button("+").clicked()
                                && self.module.instruments.len() < 128
                            {
                                self.module.instruments.push(make_empty_instrument());
                                self.selected_instrument =
                                    self.module.instruments.len() - 1;
                            }
                        });
                    });

                    // Scrollable instrument rows
                    egui::ScrollArea::vertical()
                        .id_salt("inst_scroll")
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            let n = self.module.instruments.len();
                            if n == 0 {
                                ui.label(
                                    RichText::new("(empty — press + to add)")
                                        .color(C_MUTED),
                                );
                                return;
                            }
                            for i in 0..n {
                                let is_sel = i == self.selected_instrument;
                                let name =
                                    self.module.instruments[i].name.clone();

                                let row_h = ROW_H + 2.0;
                                let (rect, resp) = ui.allocate_exact_size(
                                    Vec2::new(ui.available_width(), row_h),
                                    Sense::click(),
                                );

                                if ui.is_rect_visible(rect) {
                                    let p = ui.painter();
                                    p.rect_filled(
                                        rect,
                                        0.0,
                                        if is_sel { C_SELECTED_ROW } else { C_LIST_BG },
                                    );
                                    // 1-based index in accent colour
                                    p.text(
                                        rect.left_top() + Vec2::new(2.0, 1.0),
                                        Align2::LEFT_TOP,
                                        format!("{:02X}", i + 1),
                                        font8(),
                                        C_INST_IDX,
                                    );
                                    // Instrument name (or dim placeholder)
                                    let (display_name, text_col) = if name.is_empty() {
                                        ("(empty)", C_MUTED)
                                    } else {
                                        (name.as_str(), C_TEXT)
                                    };
                                    p.text(
                                        rect.left_top() + Vec2::new(20.0, 1.0),
                                        Align2::LEFT_TOP,
                                        display_name,
                                        font8(),
                                        text_col,
                                    );
                                }

                                if resp.clicked() {
                                    self.selected_instrument = i;
                                }
                            }
                        });
                });
            });

            ui.separator();

            // ── Right: sample list for the selected instrument ─────────────
            ui.vertical(|ui| {
                ui.label(RichText::new("Samples").color(C_CHROME_LIGHT));

                egui::ScrollArea::vertical()
                    .id_salt("sample_scroll")
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        if self.selected_instrument >= self.module.instruments.len() {
                            ui.label(
                                RichText::new("(select an instrument)").color(C_MUTED),
                            );
                            return;
                        }
                        let n =
                            self.module.instruments[self.selected_instrument].samples.len();
                        if n == 0 {
                            ui.label(RichText::new("(no samples)").color(C_MUTED));
                            return;
                        }
                        for s in 0..n {
                            let sname = self.module.instruments[self.selected_instrument]
                                .samples[s]
                                .name
                                .clone();
                            let row_h = ROW_H + 2.0;
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), row_h),
                                Sense::hover(),
                            );
                            if ui.is_rect_visible(rect) {
                                let p = ui.painter();
                                p.rect_filled(rect, 0.0, C_LIST_BG);
                                let (sname_str, col) = if sname.is_empty() {
                                    ("(unnamed)", C_MUTED)
                                } else {
                                    (sname.as_str(), C_TEXT)
                                };
                                p.text(
                                    rect.left_top() + Vec2::new(2.0, 1.0),
                                    Align2::LEFT_TOP,
                                    format!("{:02X}  {}", s, sname_str),
                                    font8(),
                                    col,
                                );
                            }
                        }
                    });
            });
        });
    }
}

// ── eframe::App ───────────────────────────────────────────────────────────────

impl eframe::App for TrackerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous repaints while playing to keep the position display live.
        if self.audio.is_playing() {
            ctx.request_repaint();
        }

        // Shared frame styles
        let chrome_frame = Frame::none()
            .fill(C_CHROME)
            .inner_margin(Margin::same(3.0));
        let panel_frame = Frame::none()
            .fill(C_PANEL_BG)
            .inner_margin(Margin::same(4.0));
        let list_frame = Frame::none()
            .fill(C_LIST_BG)
            .inner_margin(Margin::same(4.0));

        egui::TopBottomPanel::top("title_bar")
            .frame(chrome_frame)
            .show(ctx, |ui| self.show_title_bar(ui));

        egui::TopBottomPanel::top("controls_row")
            .frame(panel_frame)
            .show(ctx, |ui| self.show_controls_row(ui));

        egui::TopBottomPanel::top("menu_buttons")
            .frame(panel_frame)
            .show(ctx, |ui| self.show_menu_buttons(ui));

        egui::TopBottomPanel::top("instrument_panel")
            .min_height(80.0)
            .max_height(120.0)
            .frame(list_frame)
            .show(ctx, |ui| self.show_instrument_panel(ui));

        egui::CentralPanel::default()
            .frame(Frame::none())
            .show(ctx, |ui| {
                if self.module.patterns.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("No patterns — use Pat + to create one.")
                                .color(C_MUTED),
                        );
                    });
                    return;
                }
                // Guard against stale index after pattern removal.
                if self.current_pattern_idx >= self.module.patterns.len() {
                    self.current_pattern_idx = 0;
                }
                self.editor
                    .show(ui, &mut self.module.patterns[self.current_pattern_idx]);
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

/// Create a [`TrackerAudio`] with the appropriate backend for the current platform.
fn make_audio() -> TrackerAudio {
    #[cfg(not(target_arch = "wasm32"))]
    {
        TrackerAudio::new(Box::new(NativeAudioBackend::new()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        TrackerAudio::new(Box::new(WasmAudioBackend::new()))
    }
}

/// Construct a fresh default module: 4 channels, BPM 125, one demo pattern,
/// four named placeholder instruments.
fn default_module() -> XmModule {
    XmModule {
        name: "Untitled".to_string(),
        tracker_name: "SoymilkyTracker".to_string(),
        version: 0x0104,
        song_length: 1,
        restart_position: 0,
        channel_count: 4,
        default_tempo: 6,
        default_bpm: 125,
        linear_frequencies: true,
        pattern_order: vec![0],
        patterns: vec![demo_pattern()],
        instruments: vec![
            make_instrument("Lead Synth"),
            make_instrument("Bass Line"),
            make_instrument("Melody"),
            make_instrument("Hi-Hat"),
        ],
    }
}

/// Build a named instrument with no samples.
fn make_instrument(name: &str) -> XmInstrument {
    XmInstrument {
        name: name.to_string(),
        note_to_sample: [0u8; 96],
        volume_envelope: XmEnvelope::default(),
        panning_envelope: XmEnvelope::default(),
        volume_fadeout: 0,
        vibrato_type: 0,
        vibrato_sweep: 0,
        vibrato_depth: 0,
        vibrato_rate: 0,
        samples: vec![],
    }
}

/// Build an unnamed empty instrument (for user-added slots).
fn make_empty_instrument() -> XmInstrument {
    make_instrument("")
}

/// Double the length of a pattern by inserting an empty row after every existing row.
fn expand_pattern(pat: &mut XmPattern) {
    let ch = pat.rows.first().map(|r| r.len()).unwrap_or(0);
    let old: Vec<Vec<XmCell>> = pat.rows.drain(..).collect();
    for row in old {
        pat.rows.push(row);
        pat.rows.push(vec![XmCell::default(); ch]);
    }
}

/// Halve the length of a pattern by keeping every other row.
fn shrink_pattern(pat: &mut XmPattern) {
    let new_rows: Vec<Vec<XmCell>> = pat.rows.iter().step_by(2).cloned().collect();
    pat.rows = new_rows;
}

/// Build a 64-row × 4-channel demo pattern with representative note data.
fn demo_pattern() -> XmPattern {
    let channels = 4;
    let rows = 64;
    let mut grid: Vec<Vec<XmCell>> = vec![vec![XmCell::default(); channels]; rows];

    // Channel 0 — kick-like: C-4 every 4 rows with set-volume column
    for r in (0..rows).step_by(4) {
        grid[r][0] = XmCell {
            note: XmNote::On(49), // C-4
            instrument: 1,
            volume: 0x50,
            effect: 0,
            effect_param: 0,
        };
    }

    // Channel 1 — bass: alternating G-3 / G-4
    for (r, note) in [(8, 44u8), (16, 56), (24, 44), (32, 56), (40, 44), (48, 56), (56, 44)] {
        grid[r][1] = XmCell {
            note: XmNote::On(note),
            instrument: 2,
            volume: 0x40,
            effect: 0,
            effect_param: 0,
        };
    }

    // Channel 2 — melody with effects
    for &(r, note, fx, fp) in &[
        (0, 53u8, 0x04u8, 0x28u8), // E-4 vibrato
        (4, 55, 0x0A, 0x04),        // F#4 vol-slide up
        (8, 57, 0, 0),              // A-4
        (12, 55, 0, 0),             // F#4
        (16, 53, 0x04, 0x18),       // E-4 vibrato
        (20, 52, 0, 0),             // Eb4
        (24, 50, 0, 0),             // D-4
        (28, 48, 0, 0),             // C#4
        (32, 49, 0, 0),             // C-4
        (48, 53, 0x0C, 0x40),       // E-4 set-vol
        (56, 57, 0x0C, 0x30),       // A-4 set-vol
    ] {
        grid[r][2] = XmCell {
            note: XmNote::On(note),
            instrument: 3,
            volume: 0,
            effect: fx,
            effect_param: fp,
        };
    }

    // Channel 3 — hi-hat with occasional key-off
    for r in (2..rows).step_by(4) {
        grid[r][3] = XmCell {
            note: XmNote::On(61), // C-5
            instrument: 4,
            volume: 0x30,
            effect: 0,
            effect_param: 0,
        };
    }
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
