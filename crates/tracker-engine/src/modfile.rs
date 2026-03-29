// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! ProTracker / Amiga MOD file parser.
//!
//! Produces an [`XmModule`] so the existing [`crate::player::Player`] can
//! play MOD files without a separate engine.
//!
//! # Supported variants
//! - 4-channel `M.K.` / `M!K!` / `FLT4` (the most common)
//! - Multi-channel `NNCHNu` tag format (`6CHN`, `8CHN`, `10CH`, etc.)
//! - `FLT8`, `OCTA` (8-channel)
//! - 31-instrument files only; the obsolete 15-instrument STK format is
//!   rejected with an error.
//!
//! # Mapping to XmModule
//! | MOD concept | XmModule field |
//! |-------------|---------------|
//! | 31 instruments (sample + header) | `XmInstrument` with one `XmSample` |
//! | Cell period + finetune | `XmNote::On(note)` + `XmSample::finetune` |
//! | Effects 0–F | mapped 1-to-1 (most match XM); 9xx = sample offset |
//! | Amiga PAL frequency reference | converted to XM linear pitch via log₂ |

use std::io::Cursor;

use anyhow::{Context as _, bail};

use crate::xm::{
    EnvelopePoint, SampleLoopType, XmCell, XmEnvelope, XmInstrument, XmModule, XmNote,
    XmPattern, XmSample,
};

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse a MOD file from raw bytes, returning a playable [`XmModule`].
pub fn parse(data: &[u8]) -> anyhow::Result<XmModule> {
    parse_inner(&mut Cursor::new(data))
}

// ── Parser ────────────────────────────────────────────────────────────────────

fn parse_inner(r: &mut Cursor<&[u8]>) -> anyhow::Result<XmModule> {
    let data = *r.get_ref();

    if data.len() < 1084 {
        bail!("not a MOD file: too short");
    }

    // ── Song title ────────────────────────────────────────────────────────
    let name = read_fixed_ascii(&data[0..20]);

    // ── Instrument headers (31 × 30 bytes, starting at offset 20) ────────
    let mut instruments: Vec<RawInstr> = Vec::with_capacity(31);
    for i in 0..31 {
        let base = 20 + i * 30;
        let instr = RawInstr {
            name: read_fixed_ascii(&data[base..base + 22]),
            length_words: u16::from_be_bytes([data[base + 22], data[base + 23]]),
            finetune_nibble: data[base + 24] & 0x0F,
            volume: data[base + 25].min(64),
            loop_start_words: u16::from_be_bytes([data[base + 26], data[base + 27]]),
            loop_len_words: u16::from_be_bytes([data[base + 28], data[base + 29]]),
        };
        instruments.push(instr);
    }

    // ── Song header ───────────────────────────────────────────────────────
    let song_length = data[950] as usize;
    if song_length == 0 || song_length > 128 {
        bail!("MOD song_length out of range: {song_length}");
    }
    // data[951] = restart position (often 0x7F, ignored on initial load)
    let restart_position = if data[951] < song_length as u8 {
        data[951] as u16
    } else {
        0
    };
    let order_table = &data[952..1080];
    let pattern_order: Vec<u8> = order_table[..song_length].to_vec();

    // ── Magic / channel count ─────────────────────────────────────────────
    let magic: [u8; 4] = data[1080..1084].try_into().unwrap();
    let channels = detect_channels(&magic)
        .with_context(|| format!("unsupported MOD format: {:?}", std::str::from_utf8(&magic)))?;

    // ── Patterns ──────────────────────────────────────────────────────────
    let n_patterns = (*pattern_order.iter().max().unwrap_or(&0) as usize) + 1;
    let bytes_per_row = channels * 4;
    let bytes_per_pattern = 64 * bytes_per_row;
    let pattern_data_start = 1084;
    let pattern_data_end = pattern_data_start + n_patterns * bytes_per_pattern;

    if data.len() < pattern_data_end {
        bail!(
            "MOD file truncated: need {} bytes for patterns, have {}",
            pattern_data_end,
            data.len()
        );
    }

    let mut patterns = Vec::with_capacity(n_patterns);
    for pi in 0..n_patterns {
        let base = pattern_data_start + pi * bytes_per_pattern;
        let mut rows: Vec<Vec<XmCell>> = Vec::with_capacity(64);
        for ri in 0..64 {
            let row_base = base + ri * bytes_per_row;
            let mut row: Vec<XmCell> = Vec::with_capacity(channels);
            for ci in 0..channels {
                let cb = row_base + ci * 4;
                let b0 = data[cb];
                let b1 = data[cb + 1];
                let b2 = data[cb + 2];
                let b3 = data[cb + 3];

                let instr_1indexed = (b0 >> 4) | (b2 >> 4 & 0x10); // high + low nibbles
                let period = (((b0 & 0x0F) as u16) << 8) | b1 as u16;
                let effect = b2 & 0x0F;
                let param = b3;

                let note = if period == 0 {
                    XmNote::None
                } else {
                    XmNote::On(period_to_xm_note(period))
                };

                row.push(XmCell {
                    note,
                    instrument: instr_1indexed,
                    volume: 0,
                    effect: map_effect(effect),
                    effect_param: param,
                });
            }
            rows.push(row);
        }
        patterns.push(XmPattern { rows });
    }

    // ── Sample data ───────────────────────────────────────────────────────
    let mut sample_offset = pattern_data_end;
    let mut xm_instruments: Vec<XmInstrument> = Vec::with_capacity(31);

    for raw in &instruments {
        let byte_len = raw.length_words as usize * 2;
        let loop_start_bytes = raw.loop_start_words as u32 * 2;
        let loop_len_bytes = raw.loop_len_words as u32 * 2;

        let loop_type = if raw.loop_len_words > 1 {
            SampleLoopType::Forward
        } else {
            SampleLoopType::None
        };

        // Read raw 8-bit signed PCM and left-shift to 16-bit.
        let end = (sample_offset + byte_len).min(data.len());
        let raw_bytes = &data[sample_offset..end];
        let pcm16: Vec<i16> = raw_bytes
            .iter()
            .map(|&b| (b as i8 as i16) << 8)
            .collect();
        sample_offset += byte_len;

        let finetune = nibble_finetune(raw.finetune_nibble);

        let sample = XmSample {
            name: raw.name.clone(),
            loop_start: loop_start_bytes,
            loop_length: loop_len_bytes,
            loop_type,
            volume: raw.volume,
            finetune,
            panning: 128,
            relative_note: 0,
            data: pcm16,
        };

        // Build a minimal XmInstrument wrapping this sample.
        let mut instr = XmInstrument {
            name: raw.name.clone(),
            note_to_sample: [0u8; 96],
            volume_envelope: XmEnvelope {
                points: vec![
                    EnvelopePoint { tick: 0, value: 64 },
                    EnvelopePoint { tick: 1, value: 64 },
                ],
                sustain_point: 0,
                loop_start: 0,
                loop_end: 0,
                enabled: false,
                sustain: false,
                looped: false,
            },
            panning_envelope: XmEnvelope::default(),
            volume_fadeout: 0,
            vibrato_type: 0,
            vibrato_sweep: 0,
            vibrato_depth: 0,
            vibrato_rate: 0,
            samples: vec![sample],
        };
        instr.note_to_sample = [0u8; 96]; // all notes → sample 0
        xm_instruments.push(instr);
    }

    Ok(XmModule {
        name,
        tracker_name: "ProTracker".into(),
        version: 0x0104,
        song_length: song_length as u16,
        restart_position,
        channel_count: channels as u16,
        default_tempo: 6,
        default_bpm: 125,
        linear_frequencies: true,
        pattern_order,
        patterns,
        instruments: xm_instruments,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Raw instrument data as read from the file before conversion.
struct RawInstr {
    name: String,
    length_words: u16,
    finetune_nibble: u8,
    volume: u8,
    loop_start_words: u16,
    loop_len_words: u16,
}

/// Detect channel count from the 4-byte format magic at offset 1080.
fn detect_channels(magic: &[u8; 4]) -> Option<usize> {
    match magic {
        b"M.K." | b"M!K!" | b"FLT4" | b"4CHN" => Some(4),
        b"2CHN" => Some(2),
        b"6CHN" => Some(6),
        b"8CHN" | b"FLT8" | b"OCTA" | b"CD81" => Some(8),
        _ => {
            // "NNCHNu pattern: e.g. "10CH", "12CH", "16CH", "32CH"
            if magic[2..4] == *b"CH" && magic[0].is_ascii_digit() && magic[1].is_ascii_digit() {
                let n = (magic[0] - b'0') as usize * 10 + (magic[1] - b'0') as usize;
                if (2..=32).contains(&n) {
                    return Some(n);
                }
            }
            None
        }
    }
}

/// Convert a MOD Amiga period value to an XM 1-indexed note number.
///
/// Uses the PAL reference: `freq = 3_546_895 / period`.
/// XM pitch is derived via `C-5 = 8363 Hz = pitch 60.0`.
fn period_to_xm_note(period: u16) -> u8 {
    let freq = 3_546_895.0_f64 / period as f64;
    let pitch = 60.0 + 12.0 * (freq / 8363.0_f64).log2();
    (pitch.round() as i32 + 1).clamp(1, 96) as u8
}

/// Convert a MOD 4-bit finetune nibble (0–15) to an XM i8 finetune value.
///
/// MOD encoding: 0–7 = +0 to +7/8 semitone; 8–15 = −8/8 to −1/8 semitone.
/// XM encoding: 1 unit = 1/128 semitone.
/// Scale factor: 1 MOD step = 1/8 semitone = 16 XM units.
fn nibble_finetune(nibble: u8) -> i8 {
    let signed: i8 = if nibble <= 7 {
        nibble as i8
    } else {
        nibble as i8 - 16
    };
    signed.saturating_mul(16)
}

/// Map a MOD effect nibble (0x0–0xF) to an XM effect byte.
///
/// Most MOD effects map directly to the same XM byte.  Effect 0xE (extended)
/// is passed through unchanged because XM uses the same Exy encoding.
fn map_effect(effect: u8) -> u8 {
    // Effects 0–F map 1-to-1 to XM effects 0x00–0x0F.
    // XM uses the same bytes for Bxx (order jump), Cxx (set vol), Dxx (break),
    // Fxx (speed/BPM), Exy extended effects, and all common portamento/arpeggio.
    effect
}

/// Read a null-padded ASCII string from a fixed-width byte slice.
fn read_fixed_ascii(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim_end().to_string()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_to_note_c5() {
        // MOD C-3 (period 428) ≈ XM C-5 (note 61, pitch 60.0).
        let note = period_to_xm_note(428);
        assert!(
            (60..=62).contains(&note),
            "period 428 should map near C-5 (notes 60-62), got {note}"
        );
    }

    #[test]
    fn period_to_note_octave_consistency() {
        // Halving the period doubles the frequency → one octave up (12 semitones).
        let n_lo = period_to_xm_note(856) as i32;
        let n_hi = period_to_xm_note(428) as i32;
        assert_eq!(n_hi - n_lo, 12, "halving period should raise note by 12 semitones");
    }

    #[test]
    fn nibble_finetune_positive() {
        assert_eq!(nibble_finetune(0), 0);
        assert_eq!(nibble_finetune(7), 7 * 16);
    }

    #[test]
    fn nibble_finetune_negative() {
        assert_eq!(nibble_finetune(8), -8 * 16); // = -128
        assert_eq!(nibble_finetune(15), -16);
    }

    #[test]
    fn detect_channels_known_magic() {
        assert_eq!(detect_channels(b"M.K."), Some(4));
        assert_eq!(detect_channels(b"8CHN"), Some(8));
        assert_eq!(detect_channels(b"6CHN"), Some(6));
    }

    #[test]
    fn detect_channels_nn_format() {
        assert_eq!(detect_channels(b"16CH"), Some(16));
        assert_eq!(detect_channels(b"32CH"), Some(32));
    }

    #[test]
    fn rejects_short_data() {
        let result = parse(&[0u8; 100]);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_unknown_magic() {
        let mut data = vec![0u8; 2000];
        data[1080..1084].copy_from_slice(b"XXXX");
        let result = parse(&data);
        assert!(result.is_err());
    }
}
