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
            scroll_to_cursor: false,
        }
    }
}

impl PatternEditor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the pattern editor for `pattern` inside `ui`.
    pub fn show(&mut self, ui: &mut egui::Ui, pattern: &XmPattern) {
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

        let font_id = FontId::new(8.0, FontFamily::Name("tracker".into()));
        let content_w = ROWNUM_W + channel_count as f32 * CHANNEL_W;
        let content_h = HEADER_H + row_count as f32 * ROW_H;

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
}
