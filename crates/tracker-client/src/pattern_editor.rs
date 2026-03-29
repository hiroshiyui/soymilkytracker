// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Pattern editor grid widget.
//!
//! Renders a [`tracker_engine::xm::XmPattern`] as a scrollable pixel-art grid
//! with per-channel columns (note, instrument, volume, effect) and a movable
//! cursor, faithful to the MilkyTracker classic layout.
//!
//! # Column layout (87 px per channel at IBM EGA 8×8)
//! ```text
//! offset →   0   24  25  33  34  42  43  51  52  60  61  69  70  78  79  86
//!            ┌────────┐│┌──────┐│┌──────┐│┌──────┐│┌──────┐│┌──────┐│┌──────┐
//!            │  Note  │││InsHi│││InsLo │││VolHi │││VolLo │││FxLtr │││OpHi  ││OpLo
//!            │ 3 chars│││1 chr│││1 chr │││1 chr │││1 chr │││1 chr │││1 chr ││1 chr
//!            └────────┘│└──────┘│└──────┘│└──────┘│└──────┘│└──────┘│└──────┘
//!                      sep      sep       sep       sep       sep      sep   sep
//! ```

use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Sense, Vec2};
use tracker_engine::xm::{XmCell, XmNote, XmPattern};

// ── Colour palette (Classic MilkyTracker) ─────────────────────────────────────

const C_BG: Color32 = Color32::from_rgb(0, 0, 0);
const C_NOTE: Color32 = Color32::from_rgb(255, 255, 255);
const C_INST: Color32 = Color32::from_rgb(128, 224, 255);
const C_VOL: Color32 = Color32::from_rgb(128, 255, 128);
const C_FX_LTR: Color32 = Color32::from_rgb(255, 128, 224);
const C_FX_OP: Color32 = Color32::from_rgb(255, 224, 128);
const C_EMPTY: Color32 = Color32::from_rgb(64, 64, 64);
const C_CURSOR_CELL: Color32 = Color32::from_rgb(128, 128, 255);
const C_CURSOR_ROW: Color32 = Color32::from_rgb(96, 32, 64);
const C_BEAT_PRI_BG: Color32 = Color32::from_rgb(32, 32, 32);
const C_BEAT_SEC_BG: Color32 = Color32::from_rgb(16, 16, 16);
const C_BEAT_PRI_FG: Color32 = Color32::from_rgb(255, 255, 0);
const C_BEAT_SEC_FG: Color32 = Color32::from_rgb(255, 255, 128);
const C_ROWNUM_FG: Color32 = Color32::from_rgb(255, 255, 255);
const C_CHROME: Color32 = Color32::from_rgb(64, 96, 128);
const C_HEADER_BG: Color32 = Color32::from_rgb(64, 96, 128);
const C_HEADER_TOP: Color32 = Color32::from_rgb(100, 140, 180);

// ── Geometry constants ────────────────────────────────────────────────────────

/// Height of one row and width of one character glyph (IBM EGA 8×8 at 8 px).
pub const ROW_H: f32 = 8.0;
pub const CHAR_W: f32 = 8.0;

/// Width of the row-number margin (2 hex digits = 16 px).
const ROWNUM_W: f32 = 16.0;

/// Total width of one channel column (10 chars × 8 px + 7 separators × 1 px = 87 px).
pub const CHANNEL_W: f32 = 87.0;

/// Height of the per-channel header row.
const HEADER_H: f32 = 12.0;

/// Width of a vertical channel separator.
const SEP_W: f32 = 1.0;

// Sub-column x-offsets within a channel (pixels from channel left edge).
const NOTE_X: f32 = 0.0; // 3 chars (0..24)
const INS_HI_X: f32 = 25.0; // 1 char  (25..33)
const INS_LO_X: f32 = 34.0; // 1 char  (34..42)
const VOL_HI_X: f32 = 43.0; // 1 char  (43..51)
const VOL_LO_X: f32 = 52.0; // 1 char  (52..60)
const FX_LTR_X: f32 = 61.0; // 1 char  (61..69)
const OP_HI_X: f32 = 70.0; // 1 char  (70..78)
const OP_LO_X: f32 = 79.0; // 1 char  (79..87)

// ── Sub-column cursor position ────────────────────────────────────────────────

/// One addressable sub-column within a pattern cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubCol {
    Note,
    InsHi,
    InsLo,
    VolHi,
    VolLo,
    FxLtr,
    OpHi,
    OpLo,
}

impl SubCol {
    fn x_offset(self) -> f32 {
        match self {
            SubCol::Note => NOTE_X,
            SubCol::InsHi => INS_HI_X,
            SubCol::InsLo => INS_LO_X,
            SubCol::VolHi => VOL_HI_X,
            SubCol::VolLo => VOL_LO_X,
            SubCol::FxLtr => FX_LTR_X,
            SubCol::OpHi => OP_HI_X,
            SubCol::OpLo => OP_LO_X,
        }
    }

    fn width(self) -> f32 {
        match self {
            SubCol::Note => 3.0 * CHAR_W,
            _ => CHAR_W,
        }
    }

    /// Next sub-column within the same channel, or `self` if already at the end.
    fn next(self) -> SubCol {
        match self {
            SubCol::Note => SubCol::InsHi,
            SubCol::InsHi => SubCol::InsLo,
            SubCol::InsLo => SubCol::VolHi,
            SubCol::VolHi => SubCol::VolLo,
            SubCol::VolLo => SubCol::FxLtr,
            SubCol::FxLtr => SubCol::OpHi,
            SubCol::OpHi => SubCol::OpLo,
            SubCol::OpLo => SubCol::OpLo,
        }
    }

    /// Previous sub-column within the same channel, or `self` if already at start.
    fn prev(self) -> SubCol {
        match self {
            SubCol::OpLo => SubCol::OpHi,
            SubCol::OpHi => SubCol::FxLtr,
            SubCol::FxLtr => SubCol::VolLo,
            SubCol::VolLo => SubCol::VolHi,
            SubCol::VolHi => SubCol::InsLo,
            SubCol::InsLo => SubCol::InsHi,
            SubCol::InsHi => SubCol::Note,
            SubCol::Note => SubCol::Note,
        }
    }
}

// ── PatternEditor ─────────────────────────────────────────────────────────────

/// Cursor and display state for the pattern editor grid.
///
/// Call [`PatternEditor::show`] each frame from inside an egui `Ui`.
pub struct PatternEditor {
    /// Row the cursor is on (0-indexed).
    pub cursor_row: usize,
    /// Channel column the cursor is on (0-indexed).
    pub cursor_channel: usize,
    /// Sub-column within the channel cell.
    pub cursor_col: SubCol,
    /// When `true`, draw the cursor row in record-mode colour.
    pub record_mode: bool,
    /// Base octave for note entry (0–7).  Default `4`.
    pub octave: u8,
    /// Number of rows the cursor advances after entering a note (0 = no advance).
    pub step: usize,
    /// Request a scroll-to-cursor on the next frame.
    scroll_to_cursor: bool,
}

impl Default for PatternEditor {
    fn default() -> Self {
        Self {
            cursor_row: 0,
            cursor_channel: 0,
            cursor_col: SubCol::Note,
            record_mode: false,
            octave: 4,
            step: 1,
            scroll_to_cursor: false,
        }
    }
}

impl PatternEditor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the pattern editor for `pattern` inside `ui`.
    pub fn show(&mut self, ui: &mut egui::Ui, pattern: &mut XmPattern) {
        let row_count = pattern.rows.len();
        let channel_count = pattern.rows.first().map(|r| r.len()).unwrap_or(0);

        if row_count == 0 || channel_count == 0 {
            ui.label("(empty pattern)");
            return;
        }

        // Clamp cursor in case the pattern size changed.
        self.cursor_row = self.cursor_row.min(row_count - 1);
        self.cursor_channel = self.cursor_channel.min(channel_count - 1);

        // Process keyboard input before rendering.
        self.handle_keys(ui, row_count, channel_count);
        self.handle_entry(ui, pattern, row_count);

        let font_id = FontId::new(8.0, FontFamily::Name("tracker".into()));
        // Minimum width required to display all channels; expand to fill the
        // viewport so the background covers the whole central panel when there
        // are fewer channels than can fill the window.
        let channels_w = ROWNUM_W + channel_count as f32 * CHANNEL_W;
        let content_w = channels_w.max(ui.available_width());
        // Similarly, ensure the painter fills the full viewport height so the
        // black background extends past the last row when the pattern is short.
        let rows_h = HEADER_H + row_count as f32 * ROW_H;
        let content_h = rows_h.max(ui.available_height());

        // Consume the flag before entering the ScrollArea closure so we can
        // pass its value without borrowing self across the closure boundary.
        let do_scroll = self.scroll_to_cursor;
        self.scroll_to_cursor = false;

        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let (response, painter) =
                    ui.allocate_painter(Vec2::new(content_w, content_h), Sense::click());
                let o = response.rect.min; // screen-space origin of the content area

                // Background
                painter.rect_filled(response.rect, 0.0, C_BG);

                // Channel headers
                for ch in 0..channel_count {
                    let ch_left = o.x + ROWNUM_W + ch as f32 * CHANNEL_W;
                    let hdr = Rect::from_min_size(
                        Pos2::new(ch_left, o.y),
                        Vec2::new(CHANNEL_W - SEP_W, HEADER_H),
                    );
                    painter.rect_filled(hdr, 0.0, C_HEADER_BG);
                    // Lighter top edge for a subtle depth effect.
                    painter.rect_filled(
                        Rect::from_min_size(hdr.min, Vec2::new(hdr.width(), 1.0)),
                        0.0,
                        C_HEADER_TOP,
                    );
                    painter.text(
                        hdr.center(),
                        Align2::CENTER_CENTER,
                        (ch + 1).to_string(),
                        font_id.clone(),
                        Color32::WHITE,
                    );
                    // Vertical separator to the right of this channel.
                    painter.rect_filled(
                        Rect::from_min_size(
                            Pos2::new(ch_left + CHANNEL_W - SEP_W, o.y),
                            Vec2::new(SEP_W, HEADER_H),
                        ),
                        0.0,
                        C_CHROME,
                    );
                }

                // Rows
                for row in 0..row_count {
                    let row_top = o.y + HEADER_H + row as f32 * ROW_H;
                    let is_cursor = row == self.cursor_row;

                    let (row_bg, rownum_fg) = if is_cursor {
                        (C_CURSOR_ROW, Color32::WHITE)
                    } else if row % 8 == 0 {
                        (C_BEAT_SEC_BG, C_BEAT_SEC_FG)
                    } else if row % 4 == 0 {
                        (C_BEAT_PRI_BG, C_BEAT_PRI_FG)
                    } else {
                        (C_BG, C_ROWNUM_FG)
                    };

                    // Row-number margin
                    painter.rect_filled(
                        Rect::from_min_size(Pos2::new(o.x, row_top), Vec2::new(ROWNUM_W, ROW_H)),
                        0.0,
                        row_bg,
                    );
                    painter.text(
                        Pos2::new(o.x, row_top),
                        Align2::LEFT_TOP,
                        format!("{:02X}", row),
                        font_id.clone(),
                        rownum_fg,
                    );

                    // Full-width cursor-row band across all channels
                    if is_cursor {
                        painter.rect_filled(
                            Rect::from_min_size(
                                Pos2::new(o.x + ROWNUM_W, row_top),
                                Vec2::new(channel_count as f32 * CHANNEL_W, ROW_H),
                            ),
                            0.0,
                            C_CURSOR_ROW,
                        );
                    }

                    // Channel cells
                    for ch in 0..channel_count {
                        let ch_left = o.x + ROWNUM_W + ch as f32 * CHANNEL_W;

                        // Beat-row background for non-cursor rows
                        if !is_cursor {
                            let cell_bg = if row % 8 == 0 {
                                C_BEAT_SEC_BG
                            } else if row % 4 == 0 {
                                C_BEAT_PRI_BG
                            } else {
                                C_BG
                            };
                            painter.rect_filled(
                                Rect::from_min_size(
                                    Pos2::new(ch_left, row_top),
                                    Vec2::new(CHANNEL_W - SEP_W, ROW_H),
                                ),
                                0.0,
                                cell_bg,
                            );
                        }

                        // Cursor-cell highlight
                        if is_cursor && ch == self.cursor_channel {
                            let sub_x = ch_left + self.cursor_col.x_offset();
                            painter.rect_filled(
                                Rect::from_min_size(
                                    Pos2::new(sub_x, row_top),
                                    Vec2::new(self.cursor_col.width(), ROW_H),
                                ),
                                0.0,
                                C_CURSOR_CELL,
                            );
                        }

                        // Cell content
                        paint_cell(
                            &painter,
                            &font_id,
                            Pos2::new(ch_left, row_top),
                            &pattern.rows[row][ch],
                        );

                        // Vertical channel separator
                        painter.rect_filled(
                            Rect::from_min_size(
                                Pos2::new(ch_left + CHANNEL_W - SEP_W, row_top),
                                Vec2::new(SEP_W, ROW_H),
                            ),
                            0.0,
                            C_CHROME,
                        );
                    }
                }

                // Click to reposition cursor
                if response.clicked()
                    && let Some(ptr) = response.interact_pointer_pos()
                {
                    let rel_y = ptr.y - o.y - HEADER_H;
                    let rel_x = ptr.x - o.x - ROWNUM_W;
                    if rel_y >= 0.0 && rel_x >= 0.0 {
                        let r = (rel_y / ROW_H) as usize;
                        let c = (rel_x / CHANNEL_W) as usize;
                        if r < row_count && c < channel_count {
                            self.cursor_row = r;
                            self.cursor_channel = c;
                            self.cursor_col = subcol_at_x(rel_x - c as f32 * CHANNEL_W);
                        }
                    }
                }

                // Scroll the cursor row into view when requested.
                if do_scroll {
                    let cursor_screen_y = o.y + HEADER_H + self.cursor_row as f32 * ROW_H;
                    ui.scroll_to_rect(
                        Rect::from_min_size(
                            Pos2::new(o.x, cursor_screen_y),
                            Vec2::new(content_w, ROW_H),
                        ),
                        Some(egui::Align::Center),
                    );
                }
            });
    }

    /// Dispatch note / data entry key events into the pattern.
    fn handle_entry(&mut self, ui: &mut egui::Ui, pattern: &mut XmPattern, row_count: usize) {
        let events: Vec<egui::Event> = ui.input(|i| i.events.clone());
        for event in &events {
            if let egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
            {
                if modifiers.ctrl || modifiers.alt || modifiers.mac_cmd {
                    continue;
                }
                self.handle_key_event(*key, pattern, row_count);
            }
        }
    }

    fn handle_key_event(&mut self, key: egui::Key, pattern: &mut XmPattern, row_count: usize) {
        let cell = &mut pattern.rows[self.cursor_row][self.cursor_channel];

        match self.cursor_col {
            SubCol::Note => {
                if key == egui::Key::Delete {
                    *cell = XmCell::default();
                    self.advance_row(row_count);
                    return;
                }
                if key == egui::Key::Num1 {
                    cell.note = XmNote::Off;
                    self.advance_row(row_count);
                    return;
                }
                if let Some(note) = qwerty_to_note(key, self.octave) {
                    cell.note = XmNote::On(note);
                    self.advance_row(row_count);
                }
            }
            SubCol::InsHi => {
                if let Some(n) = key_to_hex_nibble(key) {
                    let lo = cell.instrument & 0x0F;
                    cell.instrument = (n << 4) | lo;
                    self.cursor_col = SubCol::InsLo;
                }
            }
            SubCol::InsLo => {
                if let Some(n) = key_to_hex_nibble(key) {
                    let hi = cell.instrument & 0xF0;
                    cell.instrument = hi | n;
                    self.cursor_col = SubCol::VolHi;
                    self.advance_row(row_count);
                }
            }
            SubCol::VolHi => {
                if let Some(n) = key_to_hex_nibble(key) {
                    let lo = cell.volume & 0x0F;
                    cell.volume = (n << 4) | lo;
                    self.cursor_col = SubCol::VolLo;
                }
            }
            SubCol::VolLo => {
                if let Some(n) = key_to_hex_nibble(key) {
                    let hi = cell.volume & 0xF0;
                    cell.volume = hi | n;
                    self.cursor_col = SubCol::FxLtr;
                    self.advance_row(row_count);
                }
            }
            SubCol::FxLtr => {
                if let Some(n) = key_to_hex_nibble(key) {
                    cell.effect = n;
                    self.cursor_col = SubCol::OpHi;
                }
            }
            SubCol::OpHi => {
                if let Some(n) = key_to_hex_nibble(key) {
                    let lo = cell.effect_param & 0x0F;
                    cell.effect_param = (n << 4) | lo;
                    self.cursor_col = SubCol::OpLo;
                }
            }
            SubCol::OpLo => {
                if let Some(n) = key_to_hex_nibble(key) {
                    let hi = cell.effect_param & 0xF0;
                    cell.effect_param = hi | n;
                    self.cursor_col = SubCol::Note;
                    self.advance_row(row_count);
                }
            }
        }
    }

    /// Advance the cursor by `self.step` rows, wrapping around.  No-op when `step == 0`.
    fn advance_row(&mut self, row_count: usize) {
        if self.step > 0 && row_count > 0 {
            self.cursor_row = (self.cursor_row + self.step) % row_count;
            self.scroll_to_cursor = true;
        }
    }

    fn handle_keys(&mut self, ui: &mut egui::Ui, row_count: usize, channel_count: usize) {
        let (up, down, left, right, tab_fwd, tab_back, home, end, pgup, pgdn) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::Tab) && !i.modifiers.shift,
                i.key_pressed(egui::Key::Tab) && i.modifiers.shift,
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
                i.key_pressed(egui::Key::PageUp),
                i.key_pressed(egui::Key::PageDown),
            )
        });

        if up && self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.scroll_to_cursor = true;
        }
        if down && self.cursor_row + 1 < row_count {
            self.cursor_row += 1;
            self.scroll_to_cursor = true;
        }
        if left {
            let prev = self.cursor_col.prev();
            if prev == self.cursor_col {
                if self.cursor_channel > 0 {
                    self.cursor_channel -= 1;
                    self.cursor_col = SubCol::OpLo;
                    self.scroll_to_cursor = true;
                }
            } else {
                self.cursor_col = prev;
            }
        }
        if right {
            let next = self.cursor_col.next();
            if next == self.cursor_col {
                if self.cursor_channel + 1 < channel_count {
                    self.cursor_channel += 1;
                    self.cursor_col = SubCol::Note;
                    self.scroll_to_cursor = true;
                }
            } else {
                self.cursor_col = next;
            }
        }
        if tab_fwd {
            if self.cursor_channel + 1 < channel_count {
                self.cursor_channel += 1;
            }
            self.cursor_col = SubCol::Note;
            self.scroll_to_cursor = true;
        }
        if tab_back {
            if self.cursor_channel > 0 {
                self.cursor_channel -= 1;
            }
            self.cursor_col = SubCol::Note;
            self.scroll_to_cursor = true;
        }
        if home {
            self.cursor_row = 0;
            self.scroll_to_cursor = true;
        }
        if end {
            self.cursor_row = row_count.saturating_sub(1);
            self.scroll_to_cursor = true;
        }
        if pgup {
            self.cursor_row = self.cursor_row.saturating_sub(16);
            self.scroll_to_cursor = true;
        }
        if pgdn {
            self.cursor_row = (self.cursor_row + 16).min(row_count.saturating_sub(1));
            self.scroll_to_cursor = true;
        }
    }
}

// ── Cell rendering ────────────────────────────────────────────────────────────

fn paint_cell(painter: &egui::Painter, font_id: &FontId, origin: Pos2, cell: &XmCell) {
    // Note
    let (note_str, note_fg) = match &cell.note {
        XmNote::None => ("···".to_string(), C_EMPTY),
        XmNote::Off => ("^^^".to_string(), C_NOTE),
        XmNote::On(n) => (note_name(*n), C_NOTE),
    };
    painter.text(
        Pos2::new(origin.x + NOTE_X, origin.y),
        Align2::LEFT_TOP,
        note_str,
        font_id.clone(),
        note_fg,
    );

    // Instrument (split into Hi / Lo digit for sub-column cursor granularity)
    if cell.instrument == 0 {
        painter.text(
            Pos2::new(origin.x + INS_HI_X, origin.y),
            Align2::LEFT_TOP,
            "·",
            font_id.clone(),
            C_EMPTY,
        );
        painter.text(
            Pos2::new(origin.x + INS_LO_X, origin.y),
            Align2::LEFT_TOP,
            "·",
            font_id.clone(),
            C_EMPTY,
        );
    } else {
        let s = format!("{:02X}", cell.instrument);
        painter.text(
            Pos2::new(origin.x + INS_HI_X, origin.y),
            Align2::LEFT_TOP,
            &s[0..1],
            font_id.clone(),
            C_INST,
        );
        painter.text(
            Pos2::new(origin.x + INS_LO_X, origin.y),
            Align2::LEFT_TOP,
            &s[1..2],
            font_id.clone(),
            C_INST,
        );
    }

    // Volume
    if cell.volume == 0 {
        painter.text(
            Pos2::new(origin.x + VOL_HI_X, origin.y),
            Align2::LEFT_TOP,
            "·",
            font_id.clone(),
            C_EMPTY,
        );
        painter.text(
            Pos2::new(origin.x + VOL_LO_X, origin.y),
            Align2::LEFT_TOP,
            "·",
            font_id.clone(),
            C_EMPTY,
        );
    } else {
        let s = format!("{:02X}", cell.volume);
        painter.text(
            Pos2::new(origin.x + VOL_HI_X, origin.y),
            Align2::LEFT_TOP,
            &s[0..1],
            font_id.clone(),
            C_VOL,
        );
        painter.text(
            Pos2::new(origin.x + VOL_LO_X, origin.y),
            Align2::LEFT_TOP,
            &s[1..2],
            font_id.clone(),
            C_VOL,
        );
    }

    // Effect: letter + 2-digit parameter
    if cell.effect == 0 && cell.effect_param == 0 {
        for x in [FX_LTR_X, OP_HI_X, OP_LO_X] {
            painter.text(
                Pos2::new(origin.x + x, origin.y),
                Align2::LEFT_TOP,
                "·",
                font_id.clone(),
                C_EMPTY,
            );
        }
    } else {
        let param = format!("{:02X}", cell.effect_param);
        painter.text(
            Pos2::new(origin.x + FX_LTR_X, origin.y),
            Align2::LEFT_TOP,
            effect_char(cell.effect).to_string(),
            font_id.clone(),
            C_FX_LTR,
        );
        painter.text(
            Pos2::new(origin.x + OP_HI_X, origin.y),
            Align2::LEFT_TOP,
            &param[0..1],
            font_id.clone(),
            C_FX_OP,
        );
        painter.text(
            Pos2::new(origin.x + OP_LO_X, origin.y),
            Align2::LEFT_TOP,
            &param[1..2],
            font_id.clone(),
            C_FX_OP,
        );
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Return the sub-column that contains pixel `x` (relative to channel left edge).
fn subcol_at_x(x: f32) -> SubCol {
    if x < INS_HI_X {
        SubCol::Note
    } else if x < INS_LO_X {
        SubCol::InsHi
    } else if x < VOL_HI_X {
        SubCol::InsLo
    } else if x < VOL_LO_X {
        SubCol::VolHi
    } else if x < FX_LTR_X {
        SubCol::VolLo
    } else if x < OP_HI_X {
        SubCol::FxLtr
    } else if x < OP_LO_X {
        SubCol::OpHi
    } else {
        SubCol::OpLo
    }
}

/// Convert a 1-indexed XM note number (1 = C-0 … 96 = B-7) to `"C-4"` style.
fn note_name(n: u8) -> String {
    const NAMES: [&str; 12] = [
        "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
    ];
    let idx = (n as usize).saturating_sub(1);
    format!("{}{}", NAMES[idx % 12], idx / 12)
}

/// Map an XM effect nibble to its display character (`0`–`F`).
fn effect_char(effect: u8) -> char {
    match effect {
        0x00..=0x09 => (b'0' + effect) as char,
        0x0A => 'A',
        0x0B => 'B',
        0x0C => 'C',
        0x0D => 'D',
        0x0E => 'E',
        0x0F => 'F',
        _ => '?',
    }
}

/// Map a QWERTY key to a 1-indexed XM note number using the MilkyTracker piano layout.
///
/// Lower row (Z–M + `,./;`) covers `octave` and `octave+1`.
/// Upper row (Q–P + number-row sharps) covers `octave+1` and `octave+2`.
/// Returns `None` for keys that are not part of the piano layout.
pub fn qwerty_to_note(key: egui::Key, octave: u8) -> Option<u8> {
    // (semitone 0–11, octave offset relative to base octave)
    let (semitone, oct_off): (u8, u8) = match key {
        // Lower row — white and black keys
        egui::Key::Z => (0, 0),  // C
        egui::Key::S => (1, 0),  // C#
        egui::Key::X => (2, 0),  // D
        egui::Key::D => (3, 0),  // D#
        egui::Key::C => (4, 0),  // E
        egui::Key::V => (5, 0),  // F
        egui::Key::G => (6, 0),  // F#
        egui::Key::B => (7, 0),  // G
        egui::Key::H => (8, 0),  // G#
        egui::Key::N => (9, 0),  // A
        egui::Key::J => (10, 0), // A#
        egui::Key::M => (11, 0), // B
        // Lower row overflow into octave+1
        egui::Key::Comma => (0, 1),     // C
        egui::Key::L => (1, 1),         // C#
        egui::Key::Period => (2, 1),    // D
        egui::Key::Semicolon => (3, 1), // D#
        egui::Key::Slash => (4, 1),     // E
        // Upper row — white and black keys at octave+1
        egui::Key::Q => (0, 1),     // C
        egui::Key::Num2 => (1, 1),  // C#
        egui::Key::W => (2, 1),     // D
        egui::Key::Num3 => (3, 1),  // D#
        egui::Key::E => (4, 1),     // E
        egui::Key::R => (5, 1),     // F
        egui::Key::Num5 => (6, 1),  // F#
        egui::Key::T => (7, 1),     // G
        egui::Key::Num6 => (8, 1),  // G#
        egui::Key::Y => (9, 1),     // A
        egui::Key::Num7 => (10, 1), // A#
        egui::Key::U => (11, 1),    // B
        // Upper row overflow into octave+2
        egui::Key::I => (0, 2),            // C
        egui::Key::Num9 => (1, 2),         // C#
        egui::Key::O => (2, 2),            // D
        egui::Key::Num0 => (3, 2),         // D#
        egui::Key::P => (4, 2),            // E
        egui::Key::OpenBracket => (5, 2),  // F
        egui::Key::Minus => (6, 2),        // F#
        egui::Key::CloseBracket => (7, 2), // G
        _ => return None,
    };
    let note_oct = octave as i32 + oct_off as i32;
    let note_1indexed = note_oct * 12 + semitone as i32 + 1;
    if (1..=96).contains(&note_1indexed) {
        Some(note_1indexed as u8)
    } else {
        None
    }
}

/// Map a key to a hex nibble (0x0–0xF).  Keys 0–9 map to 0–9; A–F map to 10–15.
/// Returns `None` for any other key.
pub fn key_to_hex_nibble(key: egui::Key) -> Option<u8> {
    match key {
        egui::Key::Num0 => Some(0x0),
        egui::Key::Num1 => Some(0x1),
        egui::Key::Num2 => Some(0x2),
        egui::Key::Num3 => Some(0x3),
        egui::Key::Num4 => Some(0x4),
        egui::Key::Num5 => Some(0x5),
        egui::Key::Num6 => Some(0x6),
        egui::Key::Num7 => Some(0x7),
        egui::Key::Num8 => Some(0x8),
        egui::Key::Num9 => Some(0x9),
        egui::Key::A => Some(0xA),
        egui::Key::B => Some(0xB),
        egui::Key::C => Some(0xC),
        egui::Key::D => Some(0xD),
        egui::Key::E => Some(0xE),
        egui::Key::F => Some(0xF),
        _ => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_name_c0() {
        assert_eq!(note_name(1), "C-0");
    }

    #[test]
    fn note_name_c4() {
        // C-4 = 1-indexed note 49: (49-1)/12 = 4, semitone 0 → "C-4"
        assert_eq!(note_name(49), "C-4");
    }

    #[test]
    fn note_name_fsharp5() {
        // F#5 = note 67: idx=66, octave=5, semitone=6 → "F#5"
        assert_eq!(note_name(67), "F#5");
    }

    #[test]
    fn note_name_b7() {
        assert_eq!(note_name(96), "B-7");
    }

    #[test]
    fn effect_char_hex() {
        assert_eq!(effect_char(0x00), '0');
        assert_eq!(effect_char(0x09), '9');
        assert_eq!(effect_char(0x0A), 'A');
        assert_eq!(effect_char(0x0F), 'F');
    }

    #[test]
    fn subcol_boundaries() {
        assert_eq!(subcol_at_x(0.0), SubCol::Note);
        assert_eq!(subcol_at_x(24.0), SubCol::Note); // still Note (< 25.0)
        assert_eq!(subcol_at_x(25.0), SubCol::InsHi);
        assert_eq!(subcol_at_x(34.0), SubCol::InsLo);
        assert_eq!(subcol_at_x(86.0), SubCol::OpLo);
    }

    #[test]
    fn subcol_navigate_round_trip() {
        let mut sc = SubCol::Note;
        // Go all the way right
        for _ in 0..7 {
            sc = sc.next();
        }
        assert_eq!(sc, SubCol::OpLo);
        // Go all the way back
        for _ in 0..7 {
            sc = sc.prev();
        }
        assert_eq!(sc, SubCol::Note);
    }

    #[test]
    fn subcol_stops_at_edges() {
        assert_eq!(SubCol::Note.prev(), SubCol::Note);
        assert_eq!(SubCol::OpLo.next(), SubCol::OpLo);
    }

    // ── qwerty_to_note ─────────────────────────────────────────────────────────

    #[test]
    fn qwerty_z_is_c_at_octave() {
        // Z = C at base octave; octave 4 → C-4 = note 49
        assert_eq!(qwerty_to_note(egui::Key::Z, 4), Some(49));
    }

    #[test]
    fn qwerty_s_is_csharp_at_octave() {
        // S = C# at base octave; octave 4 → C#4 = note 50
        assert_eq!(qwerty_to_note(egui::Key::S, 4), Some(50));
    }

    #[test]
    fn qwerty_q_is_c_at_octave_plus_one() {
        // Q = C at octave+1; octave 4 → C-5 = note 61
        assert_eq!(qwerty_to_note(egui::Key::Q, 4), Some(61));
    }

    #[test]
    fn qwerty_i_is_c_at_octave_plus_two() {
        // I = C at octave+2; octave 4 → C-6 = note 73
        assert_eq!(qwerty_to_note(egui::Key::I, 4), Some(73));
    }

    #[test]
    fn qwerty_out_of_range_returns_none() {
        // Octave 7: I = C-9, note = 7*12+2*12+0+1 = 109 → out of range
        assert_eq!(qwerty_to_note(egui::Key::I, 7), None);
    }

    #[test]
    fn qwerty_unrelated_key_returns_none() {
        assert_eq!(qwerty_to_note(egui::Key::F1, 4), None);
        assert_eq!(qwerty_to_note(egui::Key::Enter, 4), None);
    }

    // ── key_to_hex_nibble ──────────────────────────────────────────────────────

    #[test]
    fn hex_nibble_digits() {
        assert_eq!(key_to_hex_nibble(egui::Key::Num0), Some(0x0));
        assert_eq!(key_to_hex_nibble(egui::Key::Num9), Some(0x9));
    }

    #[test]
    fn hex_nibble_letters() {
        assert_eq!(key_to_hex_nibble(egui::Key::A), Some(0xA));
        assert_eq!(key_to_hex_nibble(egui::Key::F), Some(0xF));
    }

    #[test]
    fn hex_nibble_non_hex_returns_none() {
        assert_eq!(key_to_hex_nibble(egui::Key::Z), None);
        assert_eq!(key_to_hex_nibble(egui::Key::Tab), None);
    }
}
