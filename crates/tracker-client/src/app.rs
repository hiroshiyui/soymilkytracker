// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Main application struct and egui update loop.
//!
//! [`TrackerApp`] owns the full [`XmModule`] being edited, the audio
//! controller, and all UI-widget state.  The [`eframe::App::update`] method
//! assembles the multi-panel layout defined in `doc/ui-mockups.md`.
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
//! │  [SAMPLE EDITOR — optional bottom panel]│
//! ├─────────────────────────────────────────┤
//! │  PATTERN EDITOR  (central panel)         │
//! └─────────────────────────────────────────┘
//! ```

use std::sync::Arc;

use egui::{Align2, Color32, FontFamily, FontId, Frame, Margin, RichText, Sense, Ui, Vec2};
use tracker_engine::{
    TrackerAudio,
    xm::{SampleLoopType, XmCell, XmEnvelope, XmInstrument, XmModule, XmNote, XmPattern},
};
#[cfg(not(target_arch = "wasm32"))]
use tracker_engine::backend::NativeAudioBackend;
#[cfg(target_arch = "wasm32")]
use tracker_engine::backend::WasmAudioBackend;

use crate::pattern_editor::{PatternEditor, ROW_H};

// ── Font helpers ──────────────────────────────────────────────────────────────

const IBM_EGA_8X8: &[u8] = include_bytes!("../../../assets/fonts/Ac437_IBM_EGA_8x8.ttf");
pub const FONT_TRACKER: &str = "tracker";

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
    fonts
        .families
        .entry(egui::FontFamily::Name(FONT_TRACKER.into()))
        .or_default()
        .push(FONT_TRACKER.to_owned());
    ctx.set_fonts(fonts);
}

fn font8() -> FontId {
    FontId::new(8.0, FontFamily::Name(FONT_TRACKER.into()))
}

// ── Colour palette ────────────────────────────────────────────────────────────

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
const C_WAVEFORM: Color32 = Color32::from_rgb(255, 255, 128);
const C_LOOP_MARKER: Color32 = Color32::from_rgb(255, 128, 224);

// Maximum undo history depth.
const UNDO_LIMIT: usize = 50;

// ── TrackerApp ────────────────────────────────────────────────────────────────

pub struct TrackerApp {
    // ── Song data ─────────────────────────────────────────────────────────────
    /// The full XM module currently being composed and edited.
    module: XmModule,

    // ── Undo / redo history ───────────────────────────────────────────────────
    /// Stack of module snapshots saved before each editing action.
    undo_stack: Vec<XmModule>,
    /// Snapshots for Ctrl+Y / Ctrl+Shift+Z redo.
    redo_stack: Vec<XmModule>,

    // ── Audio ─────────────────────────────────────────────────────────────────
    audio: TrackerAudio,

    // ── Editor state ──────────────────────────────────────────────────────────
    editor: PatternEditor,
    current_pattern_idx: usize,
    current_order_idx: usize,
    /// 0-based index into `module.instruments` for the selected instrument.
    selected_instrument: usize,
    /// 0-based index into the selected instrument's samples.
    selected_sample: usize,
    record_mode: bool,

    // ── Overlay / panel toggles ───────────────────────────────────────────────
    /// Whether the sample-editor bottom panel is visible.
    show_sample_editor: bool,
    /// Whether the keyboard-shortcut help overlay is visible.
    show_help: bool,

    // ── Inline instrument-name editing ────────────────────────────────────────
    /// Index of the instrument whose name is being edited in-place, if any.
    editing_instrument_name: Option<usize>,
}

impl TrackerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        Self {
            module: default_module(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            audio: make_audio(),
            editor: PatternEditor::new(),
            current_pattern_idx: 0,
            current_order_idx: 0,
            selected_instrument: 0,
            selected_sample: 0,
            record_mode: false,
            show_sample_editor: false,
            show_help: false,
            editing_instrument_name: None,
        }
    }

    // ── Undo / redo ───────────────────────────────────────────────────────────

    /// Save the current module state to the undo stack.
    ///
    /// Only pushes if the module actually changed since the last checkpoint.
    fn checkpoint(&mut self) {
        if self.undo_stack.last() != Some(&self.module) {
            if self.undo_stack.len() >= UNDO_LIMIT {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(self.module.clone());
            self.redo_stack.clear();
        }
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            if self.redo_stack.len() >= UNDO_LIMIT {
                self.redo_stack.remove(0);
            }
            self.redo_stack.push(self.module.clone());
            self.module = prev;
            self.clamp_indices();
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            if self.undo_stack.len() >= UNDO_LIMIT {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(self.module.clone());
            self.module = next;
            self.clamp_indices();
        }
    }

    /// Clamp cursor indices after undo/redo in case the module shrank.
    fn clamp_indices(&mut self) {
        if !self.module.pattern_order.is_empty() {
            self.current_order_idx =
                self.current_order_idx.min(self.module.pattern_order.len() - 1);
        }
        if !self.module.patterns.is_empty() {
            self.current_pattern_idx =
                self.current_pattern_idx.min(self.module.patterns.len() - 1);
        }
        if !self.module.instruments.is_empty() {
            self.selected_instrument =
                self.selected_instrument.min(self.module.instruments.len() - 1);
        }
    }

    // ── Transport ─────────────────────────────────────────────────────────────

    fn play_song(&mut self) {
        let arc = Arc::new(self.module.clone());
        self.audio.load(arc);
        self.audio.seek(self.current_order_idx, 0);
        let _ = self.audio.play();
    }

    fn play_pattern(&mut self) {
        let mut m = self.module.clone();
        m.song_length = 1;
        m.restart_position = 0;
        m.pattern_order = vec![self.current_pattern_idx as u8];
        self.audio.load(Arc::new(m));
        let _ = self.audio.play();
    }

    fn stop(&mut self) {
        self.audio.stop();
    }

    // ── Native file loading ───────────────────────────────────────────────────

    /// Open a native file-picker, load a `.pat` file, and replace the
    /// currently selected instrument.  No-op on WASM (dialog not available).
    #[cfg(not(target_arch = "wasm32"))]
    fn load_instrument_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Load instrument")
            .add_filter("GUS Patch", &["pat"])
            .add_filter("SoundFont 2", &["sf2"])
            .add_filter("SoundFont 3", &["sf3"])
            .add_filter("All files", &["*"])
            .pick_file()
        else {
            return;
        };

        let Ok(bytes) = std::fs::read(&path) else {
            return;
        };

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let inst = match ext.as_str() {
            "pat" => tracker_engine::gus::parse(&bytes).ok(),
            // SF2/SF3: not yet implemented — fall through to None
            _ => None,
        };

        if let Some(loaded) = inst {
            self.checkpoint();
            // Grow the instrument list if the slot doesn't exist yet.
            while self.module.instruments.len() <= self.selected_instrument {
                self.module.instruments.push(make_empty_instrument());
            }
            self.module.instruments[self.selected_instrument] = loaded;
            self.selected_sample = 0;
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn load_instrument_file(&mut self) {
        // File picking on WASM requires async browser APIs; not yet implemented.
    }

    // ── Panel rendering ───────────────────────────────────────────────────────

    fn show_title_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("SoymilkyTracker").color(C_CHROME_LIGHT));
            ui.separator();
            ui.label(RichText::new("Title:").color(C_CHROME_LIGHT));
            ui.add(egui::TextEdit::singleline(&mut self.module.name).desired_width(160.0));
            ui.separator();

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

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let help_label = if self.show_help { "F1 ▸ Hide help" } else { "F1 ▸ Help" };
                if ui.small_button(help_label).clicked() {
                    self.show_help = !self.show_help;
                }
                let redo_enabled = !self.redo_stack.is_empty();
                let undo_enabled = !self.undo_stack.is_empty();
                ui.add_enabled_ui(redo_enabled, |ui| {
                    if ui.small_button("⟳ Redo").clicked() {
                        self.redo();
                    }
                });
                ui.add_enabled_ui(undo_enabled, |ui| {
                    if ui.small_button("⟲ Undo").clicked() {
                        self.undo();
                    }
                });
            });
        });
    }

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
                ui.label(
                    RichText::new(format!("{:02X}→{:02X}", self.current_order_idx, cur_pat))
                        .color(C_INST_IDX),
                );
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
                if ui.small_button("+").clicked() {
                    let n_pats = self.module.patterns.len().saturating_sub(1) as u8;
                    let new_pat = cur_pat.saturating_add(1).min(n_pats);
                    if self.module.pattern_order.get(self.current_order_idx).is_some() {
                        self.checkpoint();
                        self.module.pattern_order[self.current_order_idx] = new_pat;
                        self.current_pattern_idx = new_pat as usize;
                    }
                }
                if ui.small_button("−").clicked() {
                    let new_pat = cur_pat.saturating_sub(1);
                    if self.module.pattern_order.get(self.current_order_idx).is_some() {
                        self.checkpoint();
                        self.module.pattern_order[self.current_order_idx] = new_pat;
                        self.current_pattern_idx = new_pat as usize;
                    }
                }
                if ui.small_button("Ins").clicked() {
                    self.checkpoint();
                    let pos = (self.current_order_idx + 1).min(self.module.pattern_order.len());
                    self.module.pattern_order.insert(pos, cur_pat);
                    self.module.song_length = (self.module.song_length + 1).min(255);
                    self.current_order_idx = pos;
                }
                if ui.small_button("Del").clicked() && self.module.pattern_order.len() > 1 {
                    self.checkpoint();
                    self.module.pattern_order.remove(self.current_order_idx);
                    self.module.song_length = (self.module.song_length - 1).max(1);
                    if self.current_order_idx >= self.module.pattern_order.len() {
                        self.current_order_idx = self.module.pattern_order.len() - 1;
                    }
                    self.current_pattern_idx =
                        self.module.pattern_order[self.current_order_idx] as usize;
                }
            });

            // ── Speed section ──────────────────────────────────────────────
            ui.group(|ui| {
                ui.label(RichText::new("BPM").color(C_CHROME_LIGHT));
                ui.label(RichText::new(format!("{:3}", self.module.default_bpm)).color(C_TEXT));
                if ui.small_button("+").clicked() {
                    self.checkpoint();
                    self.module.default_bpm = (self.module.default_bpm + 1).min(255);
                }
                if ui.small_button("−").clicked() {
                    self.checkpoint();
                    self.module.default_bpm = (self.module.default_bpm - 1).max(1);
                }
                ui.separator();
                ui.label(RichText::new("TPB").color(C_CHROME_LIGHT));
                ui.label(
                    RichText::new(format!("{:2}", self.module.default_tempo)).color(C_TEXT),
                );
                if ui.small_button("+").clicked() {
                    self.checkpoint();
                    self.module.default_tempo = (self.module.default_tempo + 1).min(31);
                }
                if ui.small_button("−").clicked() {
                    self.checkpoint();
                    self.module.default_tempo = (self.module.default_tempo - 1).max(1);
                }
                ui.separator();
                ui.label(RichText::new("Stp").color(C_CHROME_LIGHT));
                ui.label(RichText::new(format!("{:2}", self.editor.step)).color(C_TEXT));
                if ui.small_button("+").clicked() {
                    self.editor.step = (self.editor.step + 1).min(16);
                }
                if ui.small_button("−").clicked() {
                    self.editor.step = self.editor.step.saturating_sub(1);
                }
                ui.separator();
                ui.label(RichText::new("Oct").color(C_CHROME_LIGHT));
                ui.label(RichText::new(format!("{}", self.editor.octave)).color(C_TEXT));
                if ui.small_button("+").clicked() && self.editor.octave < 8 {
                    self.editor.octave += 1;
                }
                if ui.small_button("−").clicked() && self.editor.octave > 0 {
                    self.editor.octave -= 1;
                }
            });

            // ── Pattern section ────────────────────────────────────────────
            ui.group(|ui| {
                let n_pats = self.module.patterns.len().max(1);
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
                let row_count = self
                    .module
                    .patterns
                    .get(self.current_pattern_idx)
                    .map(|p| p.rows.len())
                    .unwrap_or(64);
                ui.label(RichText::new("Rows").color(C_CHROME_LIGHT));
                ui.label(RichText::new(format!("{:3}", row_count)).color(C_TEXT));
                if ui.small_button("×2").clicked() {
                    let can_expand = self
                        .module
                        .patterns
                        .get(self.current_pattern_idx)
                        .map(|p| p.rows.len() <= 128)
                        .unwrap_or(false);
                    if can_expand {
                        self.checkpoint();
                        expand_pattern(&mut self.module.patterns[self.current_pattern_idx]);
                    }
                }
                if ui.small_button("/2").clicked() {
                    let can_shrink = self
                        .module
                        .patterns
                        .get(self.current_pattern_idx)
                        .map(|p| p.rows.len() >= 4)
                        .unwrap_or(false);
                    if can_shrink {
                        self.checkpoint();
                        shrink_pattern(&mut self.module.patterns[self.current_pattern_idx]);
                    }
                }
            });
        });
    }

    fn show_menu_buttons(&mut self, ui: &mut Ui) {
        let is_playing = self.audio.is_playing();
        ui.horizontal(|ui| {
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

            // Sample editor toggle
            let smp_text = RichText::new("Smp. Ed.")
                .color(if self.show_sample_editor { C_CHROME_LIGHT } else { C_TEXT });
            let smp_btn = egui::Button::new(smp_text)
                .fill(if self.show_sample_editor { C_PANEL_BG } else { C_BTN_DIM });
            if ui.add(smp_btn).clicked() {
                self.show_sample_editor = !self.show_sample_editor;
            }

            for label in ["Load", "Save", "Ins. Ed.", "Config", "About"] {
                let _ = ui.button(label);
            }
        });
    }

    fn show_instrument_panel(&mut self, ui: &mut Ui) {
        let half_w = (ui.available_width() / 2.0 - 4.0).max(80.0);
        let panel_h = ui.available_height();

        ui.horizontal_top(|ui| {
            // ── Left: instrument list ─────────────────────────────────────
            ui.allocate_ui(Vec2::new(half_w, panel_h), |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Instruments").color(C_CHROME_LIGHT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Load instrument file (native only)
                            if ui.small_button("Load").clicked() {
                                self.load_instrument_file();
                            }
                            if ui.small_button("−").clicked()
                                && !self.module.instruments.is_empty()
                            {
                                self.checkpoint();
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
                                self.checkpoint();
                                self.module.instruments.push(make_empty_instrument());
                                self.selected_instrument =
                                    self.module.instruments.len() - 1;
                            }
                        });
                    });

                    egui::ScrollArea::vertical()
                        .id_salt("inst_scroll")
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            let n = self.module.instruments.len();
                            if n == 0 {
                                ui.label(
                                    RichText::new("(empty — press + to add)").color(C_MUTED),
                                );
                                return;
                            }
                            for i in 0..n {
                                let is_sel = i == self.selected_instrument;
                                let is_editing = self.editing_instrument_name == Some(i);

                                let row_h = ROW_H + 2.0;

                                if is_editing {
                                    // In-place name edit
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!("{:02X}", i + 1))
                                                .color(C_INST_IDX),
                                        );
                                        let resp = ui.add(
                                            egui::TextEdit::singleline(
                                                &mut self.module.instruments[i].name,
                                            )
                                            .desired_width(ui.available_width()),
                                        );
                                        // Commit on Enter or focus loss
                                        if resp.lost_focus()
                                            || ui.input(|inp| {
                                                inp.key_pressed(egui::Key::Enter)
                                            })
                                        {
                                            self.editing_instrument_name = None;
                                        }
                                        resp.request_focus();
                                    });
                                } else {
                                    // Normal row — custom-painted for tracker aesthetic
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
                                        p.text(
                                            rect.left_top() + Vec2::new(2.0, 1.0),
                                            Align2::LEFT_TOP,
                                            format!("{:02X}", i + 1),
                                            font8(),
                                            C_INST_IDX,
                                        );
                                        let name = &self.module.instruments[i].name;
                                        let (display, col) = if name.is_empty() {
                                            ("(empty)", C_MUTED)
                                        } else {
                                            (name.as_str(), C_TEXT)
                                        };
                                        p.text(
                                            rect.left_top() + Vec2::new(20.0, 1.0),
                                            Align2::LEFT_TOP,
                                            display,
                                            font8(),
                                            col,
                                        );
                                    }
                                    if resp.clicked() {
                                        self.selected_instrument = i;
                                    }
                                    // Double-click starts inline name editing
                                    if resp.double_clicked() {
                                        self.selected_instrument = i;
                                        self.editing_instrument_name = Some(i);
                                        self.checkpoint();
                                    }
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
                            let is_sel = s == self.selected_sample;
                            let sname = self.module.instruments[self.selected_instrument]
                                .samples[s]
                                .name
                                .clone();
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
                            if resp.clicked() {
                                self.selected_sample = s;
                            }
                        }
                    });
            });
        });
    }

    /// Sample-editor bottom panel: waveform display + loop controls.
    fn show_sample_editor(&mut self, ui: &mut Ui) {
        // Resolve the currently displayed sample.
        let inst_ok = self.selected_instrument < self.module.instruments.len();
        let smp_opt = inst_ok.then(|| {
            let inst = &self.module.instruments[self.selected_instrument];
            inst.samples.get(self.selected_sample)
        }).flatten();

        ui.horizontal(|ui| {
            // Panel title
            let smp_name = smp_opt
                .map(|s| {
                    if s.name.is_empty() {
                        format!(
                            "Instrument {:02X} · Sample {:02X}",
                            self.selected_instrument + 1,
                            self.selected_sample,
                        )
                    } else {
                        s.name.clone()
                    }
                })
                .unwrap_or_else(|| "(no sample selected)".to_string());
            ui.label(
                RichText::new(format!("SAMPLE EDITOR — {smp_name}"))
                    .color(C_CHROME_LIGHT),
            );

            // Loop-type radio buttons (right-aligned)
            if let Some(smp) = smp_opt {
                // We need a mutable reference below; read what we need first.
                let _ = smp;
            }
        });

        // Waveform display area
        let wave_h = ui.available_height() - 28.0; // leave room for controls strip
        let (wave_rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), wave_h.max(40.0)),
            Sense::hover(),
        );
        let painter = ui.painter();
        painter.rect_filled(wave_rect, 0.0, Color32::BLACK);

        if let Some(smp) = smp_opt {
            let data = &smp.data;
            if !data.is_empty() {
                let n = data.len();
                let w = wave_rect.width();
                let h = wave_rect.height();
                let cy = wave_rect.center().y;
                let amplitude = h / 2.0 * 0.9;

                // Downsample: one screen pixel → one sample bucket
                let points: Vec<egui::Pos2> = (0..w as usize)
                    .map(|px| {
                        let idx = (px as f64 / w as f64 * n as f64) as usize;
                        let sample = data[idx.min(n - 1)] as f32 / i16::MAX as f32;
                        egui::Pos2::new(
                            wave_rect.left() + px as f32,
                            cy - sample * amplitude,
                        )
                    })
                    .collect();

                // Draw waveform as connected line segments
                for pair in points.windows(2) {
                    painter.line_segment([pair[0], pair[1]], (1.0, C_WAVEFORM));
                }

                // Loop start marker
                if smp.loop_type != SampleLoopType::None && smp.loop_length > 0 {
                    let ls_x = wave_rect.left()
                        + smp.loop_start as f32 / n as f32 * w;
                    let le_x = wave_rect.left()
                        + (smp.loop_start + smp.loop_length) as f32 / n as f32 * w;
                    for x in [ls_x, le_x] {
                        painter.line_segment(
                            [
                                egui::Pos2::new(x, wave_rect.top()),
                                egui::Pos2::new(x, wave_rect.bottom()),
                            ],
                            (1.0, C_LOOP_MARKER),
                        );
                    }
                }
            } else {
                painter.text(
                    wave_rect.center(),
                    Align2::CENTER_CENTER,
                    "(empty sample — no data)",
                    font8(),
                    C_MUTED,
                );
            }
        } else {
            painter.text(
                wave_rect.center(),
                Align2::CENTER_CENTER,
                "(no sample selected)",
                font8(),
                C_MUTED,
            );
        }

        // Controls strip: loop type + metadata
        ui.horizontal(|ui| {
            if let (true, Some(smp)) = (
                inst_ok,
                self.module
                    .instruments
                    .get_mut(self.selected_instrument)
                    .and_then(|inst| inst.samples.get_mut(self.selected_sample)),
            ) {
                // Loop type radio
                ui.label(RichText::new("Loop:").color(C_CHROME_LIGHT));
                let mut loop_type = smp.loop_type;
                egui::ComboBox::from_id_salt("loop_type")
                    .selected_text(match loop_type {
                        SampleLoopType::None => "None",
                        SampleLoopType::Forward => "Forward",
                        SampleLoopType::PingPong => "Ping-Pong",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut loop_type, SampleLoopType::None, "None");
                        ui.selectable_value(
                            &mut loop_type,
                            SampleLoopType::Forward,
                            "Forward",
                        );
                        ui.selectable_value(
                            &mut loop_type,
                            SampleLoopType::PingPong,
                            "Ping-Pong",
                        );
                    });
                if loop_type != smp.loop_type {
                    self.checkpoint();
                    // Re-borrow after checkpoint (borrow checker: checkpoint takes &mut self)
                    if let Some(inst) = self.module.instruments.get_mut(self.selected_instrument)
                    {
                        if let Some(s) = inst.samples.get_mut(self.selected_sample) {
                            s.loop_type = loop_type;
                        }
                    }
                }

                let smp_ro = self.module.instruments[self.selected_instrument]
                    .samples
                    .get(self.selected_sample);
                if let Some(s) = smp_ro {
                    ui.separator();
                    ui.label(
                        RichText::new(format!(
                            "Len:{:06}  LoopSt:{:06}  LoopLen:{:06}  Vol:{:02X}  Rate:{}",
                            s.data.len(),
                            s.loop_start,
                            s.loop_length,
                            s.volume,
                            s.relative_note,
                        ))
                        .color(C_TEXT),
                    );
                }
            } else {
                ui.label(RichText::new("(select instrument and sample)").color(C_MUTED));
            }
        });
    }

    /// Floating keyboard-shortcut help window (toggle with F1).
    fn show_help_window(&mut self, ctx: &egui::Context) {
        egui::Window::new("Keyboard Shortcuts")
            .id(egui::Id::new("help_window"))
            .collapsible(false)
            .resizable(true)
            .default_width(480.0)
            .show(ctx, |ui| {
                if ui.button("Close  [F1]").clicked() {
                    self.show_help = false;
                }
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    shortcut_section(ui, "Navigation", &[
                        ("↑ / ↓", "Move cursor row"),
                        ("← / →", "Move cursor sub-column"),
                        ("Tab / Shift+Tab", "Next / previous channel"),
                        ("Home / End", "Jump to first / last row"),
                        ("Page Up / Page Down", "Jump ±16 rows"),
                    ]);
                    shortcut_section(ui, "Note entry (QWERTY piano)", &[
                        ("Z X C V B N M , . /", "C D E F G A B C D E  (base octave)"),
                        ("S D   G H J", "C# D# F# G# A#  (base octave)"),
                        ("Q W E R T Y U I O P", "C D E F G A B C D E  (octave+1)"),
                        ("2 3   5 6 7", "C# D# F# G# A#  (octave+1)"),
                        ("Num1", "Key-off (^^^)"),
                        ("Delete", "Clear cell"),
                    ]);
                    shortcut_section(ui, "Data entry", &[
                        ("0–9 / A–F", "Hex digit for Inst / Vol / Fx columns"),
                    ]);
                    shortcut_section(ui, "Transport", &[
                        ("▶ Play Song", "Start song from selected order position"),
                        ("▷ Play Pat", "Loop current pattern"),
                        ("■ Stop", "Stop playback"),
                        ("● Rec", "Toggle record mode"),
                    ]);
                    shortcut_section(ui, "Undo / Redo", &[
                        ("Ctrl+Z", "Undo last action"),
                        ("Ctrl+Y  or  Ctrl+Shift+Z", "Redo"),
                    ]);
                    shortcut_section(ui, "Panels", &[
                        ("F1", "Toggle this help overlay"),
                        ("Smp. Ed. button", "Toggle sample-editor panel"),
                    ]);
                });
            });
    }
}

// ── eframe::App ───────────────────────────────────────────────────────────────

impl eframe::App for TrackerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.audio.is_playing() {
            ctx.request_repaint();
        }

        // ── Global key bindings ───────────────────────────────────────────────
        ctx.input(|inp| {
            // F1 — toggle help overlay
            if inp.key_pressed(egui::Key::F1) {
                self.show_help = !self.show_help;
            }
            // Ctrl+Z — undo
            if inp.modifiers.ctrl && inp.key_pressed(egui::Key::Z) {
                self.undo();
            }
            // Ctrl+Y or Ctrl+Shift+Z — redo
            if inp.modifiers.ctrl
                && (inp.key_pressed(egui::Key::Y)
                    || (inp.modifiers.shift && inp.key_pressed(egui::Key::Z)))
            {
                self.redo();
            }
        });

        // Show help overlay if toggled
        if self.show_help {
            self.show_help_window(ctx);
        }

        // ── Panel layout ──────────────────────────────────────────────────────
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

        // Sample editor as an optional bottom panel
        if self.show_sample_editor {
            egui::TopBottomPanel::bottom("sample_editor")
                .min_height(140.0)
                .max_height(260.0)
                .resizable(true)
                .frame(Frame::none().fill(Color32::from_rgb(16, 16, 24)).inner_margin(Margin::same(4.0)))
                .show(ctx, |ui| self.show_sample_editor(ui));
        }

        // ── Pattern editor ────────────────────────────────────────────────────
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

                // Snapshot the current pattern before the editor processes input,
                // so that note entry lands in the undo stack automatically.
                let pat_before = self.module.patterns[self.current_pattern_idx].clone();

                let idx = self.current_pattern_idx.min(self.module.patterns.len() - 1);
                self.current_pattern_idx = idx;
                self.editor
                    .show(ui, &mut self.module.patterns[idx]);

                // If the editor changed the pattern, record the pre-edit snapshot.
                if self.module.patterns[idx] != pat_before {
                    if self.undo_stack.last().map(|m| &m.patterns[idx]) != Some(&pat_before) {
                        if self.undo_stack.len() >= UNDO_LIMIT {
                            self.undo_stack.remove(0);
                        }
                        // Build a module snapshot with the pre-edit pattern.
                        let mut snap = self.module.clone();
                        snap.patterns[idx] = pat_before;
                        self.undo_stack.push(snap);
                        self.redo_stack.clear();
                    }
                }
            });
    }
}

// ── Helper functions ──────────────────────────────────────────────────────────

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

fn make_empty_instrument() -> XmInstrument {
    make_instrument("")
}

fn expand_pattern(pat: &mut XmPattern) {
    let ch = pat.rows.first().map(|r| r.len()).unwrap_or(0);
    let old: Vec<Vec<XmCell>> = pat.rows.drain(..).collect();
    for row in old {
        pat.rows.push(row);
        pat.rows.push(vec![XmCell::default(); ch]);
    }
}

fn shrink_pattern(pat: &mut XmPattern) {
    let new_rows: Vec<Vec<XmCell>> = pat.rows.iter().step_by(2).cloned().collect();
    pat.rows = new_rows;
}

/// Render a named group of shortcut rows in the help window.
fn shortcut_section(ui: &mut Ui, heading: &str, rows: &[(&str, &str)]) {
    ui.add_space(4.0);
    ui.label(RichText::new(heading).color(C_CHROME_LIGHT).strong());
    egui::Grid::new(heading)
        .num_columns(2)
        .spacing([12.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            for (key, desc) in rows {
                ui.label(RichText::new(*key).color(Color32::from_rgb(255, 224, 128)));
                ui.label(*desc);
                ui.end_row();
            }
        });
}

fn demo_pattern() -> XmPattern {
    let channels = 4;
    let rows = 64;
    let mut grid: Vec<Vec<XmCell>> = vec![vec![XmCell::default(); channels]; rows];

    for r in (0..rows).step_by(4) {
        grid[r][0] = XmCell {
            note: XmNote::On(49),
            instrument: 1,
            volume: 0x50,
            effect: 0,
            effect_param: 0,
        };
    }
    for (r, note) in [(8, 44u8), (16, 56), (24, 44), (32, 56), (40, 44), (48, 56), (56, 44)] {
        grid[r][1] = XmCell {
            note: XmNote::On(note),
            instrument: 2,
            volume: 0x40,
            effect: 0,
            effect_param: 0,
        };
    }
    for &(r, note, fx, fp) in &[
        (0, 53u8, 0x04u8, 0x28u8),
        (4, 55, 0x0A, 0x04),
        (8, 57, 0, 0),
        (12, 55, 0, 0),
        (16, 53, 0x04, 0x18),
        (20, 52, 0, 0),
        (24, 50, 0, 0),
        (28, 48, 0, 0),
        (32, 49, 0, 0),
        (48, 53, 0x0C, 0x40),
        (56, 57, 0x0C, 0x30),
    ] {
        grid[r][2] = XmCell {
            note: XmNote::On(note),
            instrument: 3,
            volume: 0,
            effect: fx,
            effect_param: fp,
        };
    }
    for r in (2..rows).step_by(4) {
        grid[r][3] = XmCell {
            note: XmNote::On(61),
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
