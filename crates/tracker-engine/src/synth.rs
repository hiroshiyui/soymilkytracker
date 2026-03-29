// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! SF2/SF3 synthesis via `oxisynth`.
//!
//! [`SfSynth`] wraps an `oxisynth::Synth` and provides a stereo-interleaved
//! fill method compatible with [`crate::backend::FillCallback`].
//!
//! ## Bundled soundfonts (all in `assets/soundfonts/`)
//!
//! | Font | Size | Embedded | Notes |
//! |------|------|----------|-------|
//! | `Open8bitVChiptuner.sf2` | 92 KB | Always | Chiptune palette (CC BY-SA 4.0) |
//! | `TimGM6mb.sf2` | 5.7 MB | Native only | Compact General MIDI default (GPL-2.0+) |
//! | `MuseScore_General.sf3` | 39 MB | Never | High-quality GM+GS (MIT); load via `load_font_bytes` |
//!
//! On WASM, `TimGM6mb.sf2` must be fetched from the server and loaded via
//! [`SfSynth::load_font_bytes`].

use std::io::Cursor;

use oxisynth::{MidiEvent, SoundFont, Synth, SynthDescriptor};

// ── Embedded soundfont bytes ──────────────────────────────────────────────────

/// Open8bitVChiptuner — 92 KB, embedded on all targets.
const CHIPTUNE_SF2: &[u8] = include_bytes!("../../../assets/soundfonts/Open8bitVChiptuner.sf2");

/// TimGM6mb — 5.7 MB, embedded on native only (too large for WASM).
#[cfg(not(target_arch = "wasm32"))]
const TIMGM6MB_SF2: &[u8] = include_bytes!("../../../assets/soundfonts/TimGM6mb.sf2");

// ── BundledFont enum ──────────────────────────────────────────────────────────

/// Identifies one of the bundled soundfonts that can be loaded by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundledFont {
    /// Open8bitVChiptuner (92 KB, CC BY-SA 4.0) — chiptune / 8-bit palette.
    /// Available on all targets.
    Open8bitVChiptuner,
    /// TimGM6mb (5.7 MB, GPL-2.0+) — compact General MIDI default.
    /// Available on native targets only; on WASM use [`SfSynth::load_font_bytes`].
    #[cfg(not(target_arch = "wasm32"))]
    TimGm6mb,
}

// ── SfSynth ───────────────────────────────────────────────────────────────────

/// Wrapper around [`oxisynth::Synth`] with a stereo-interleaved fill interface.
///
/// # Example (native)
/// ```ignore
/// let mut sf = SfSynth::new(44_100.0)?;
/// sf.load_bundled(BundledFont::TimGm6mb)?;
/// sf.program_change(0, 0)?;    // channel 0, GM bank 0, program 0 = Grand Piano
/// sf.note_on(0, 69, 100)?;     // channel 0, A4, velocity 100
///
/// let mut buf = vec![0.0f32; 2048 * 2];
/// sf.fill(&mut buf);
///
/// sf.note_off(0, 69)?;
/// ```
pub struct SfSynth {
    synth: Synth,
}

impl SfSynth {
    /// Create a new synthesiser at the given sample rate.
    ///
    /// No soundfonts are loaded; call [`load_bundled`][Self::load_bundled] or
    /// [`load_font_bytes`][Self::load_font_bytes] before sending MIDI events.
    pub fn new(sample_rate: f32) -> anyhow::Result<Self> {
        let desc = SynthDescriptor {
            sample_rate,
            ..Default::default()
        };
        let synth = Synth::new(desc).map_err(|e| anyhow::anyhow!("Synth::new: {e:?}"))?;
        Ok(Self { synth })
    }

    /// Load a bundled soundfont.
    ///
    /// On WASM only [`BundledFont::Open8bitVChiptuner`] is available; use
    /// [`load_font_bytes`][Self::load_font_bytes] for `TimGM6mb`.
    pub fn load_bundled(&mut self, font: BundledFont) -> anyhow::Result<()> {
        let bytes: &[u8] = match font {
            BundledFont::Open8bitVChiptuner => CHIPTUNE_SF2,
            #[cfg(not(target_arch = "wasm32"))]
            BundledFont::TimGm6mb => TIMGM6MB_SF2,
        };
        self.load_font_bytes(bytes)
    }

    /// Load a soundfont from raw bytes (SF2 or SF3).
    ///
    /// Use this on WASM to load `TimGM6mb.sf2` after fetching it, or to load
    /// `MuseScore_General.sf3` or any user-supplied soundfont on any target.
    pub fn load_font_bytes(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        let font = SoundFont::load(&mut Cursor::new(bytes))
            .map_err(|e| anyhow::anyhow!("SoundFont::load: {e}"))?;
        self.synth.add_font(font, true);
        Ok(())
    }

    // ── MIDI event dispatch ───────────────────────────────────────────────────

    /// Send a Note On event.
    pub fn note_on(&mut self, channel: u8, key: u8, vel: u8) -> anyhow::Result<()> {
        self.synth
            .send_event(MidiEvent::NoteOn { channel, key, vel })
            .map_err(|e| anyhow::anyhow!("NoteOn: {e:?}"))
    }

    /// Send a Note Off event.
    pub fn note_off(&mut self, channel: u8, key: u8) -> anyhow::Result<()> {
        self.synth
            .send_event(MidiEvent::NoteOff { channel, key })
            .map_err(|e| anyhow::anyhow!("NoteOff: {e:?}"))
    }

    /// Select a General MIDI program on the given channel.
    pub fn program_change(&mut self, channel: u8, program_id: u8) -> anyhow::Result<()> {
        self.synth
            .send_event(MidiEvent::ProgramChange {
                channel,
                program_id,
            })
            .map_err(|e| anyhow::anyhow!("ProgramChange: {e:?}"))
    }

    /// Silence all currently sounding notes on every channel.
    pub fn all_notes_off(&mut self) {
        for ch in 0..16u8 {
            for key in 0..128u8 {
                let _ = self
                    .synth
                    .send_event(MidiEvent::NoteOff { channel: ch, key });
            }
        }
    }

    // ── Audio rendering ───────────────────────────────────────────────────────

    /// Fill `buf` with stereo-interleaved f32 samples (`[L0, R0, L1, R1, …]`).
    ///
    /// Suitable for use as a [`crate::backend::FillCallback`] or for mixing
    /// alongside the XM [`crate::player::Player`] output.
    pub fn fill(&mut self, buf: &mut [f32]) {
        let frames = buf.len() / 2;
        for i in 0..frames {
            let (l, r) = self.synth.read_next();
            buf[i * 2] = l;
            buf[i * 2 + 1] = r;
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synth_creates_without_error() {
        let synth = SfSynth::new(44_100.0);
        assert!(synth.is_ok());
    }

    #[test]
    fn chiptune_font_loads() {
        let mut sf = SfSynth::new(44_100.0).unwrap();
        assert!(sf.load_bundled(BundledFont::Open8bitVChiptuner).is_ok());
    }

    #[test]
    fn fill_without_font_does_not_panic() {
        // oxisynth may produce tiny DSP noise even without fonts; we only
        // verify the call completes and writes to the full buffer.
        let mut sf = SfSynth::new(44_100.0).unwrap();
        let mut buf = vec![f32::NAN; 64];
        sf.fill(&mut buf);
        assert!(buf.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn note_on_off_with_chiptune_font() {
        let mut sf = SfSynth::new(44_100.0).unwrap();
        sf.load_bundled(BundledFont::Open8bitVChiptuner).unwrap();
        sf.program_change(0, 0).unwrap();
        sf.note_on(0, 60, 100).unwrap();
        let mut buf = vec![0.0f32; 256];
        sf.fill(&mut buf);
        sf.note_off(0, 60).unwrap();
        // At least some output should be non-zero after a note-on.
        assert!(
            buf.iter().any(|&x| x != 0.0),
            "expected non-silent output after note-on"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn timgm6mb_font_loads() {
        let mut sf = SfSynth::new(44_100.0).unwrap();
        assert!(sf.load_bundled(BundledFont::TimGm6mb).is_ok());
    }
}
