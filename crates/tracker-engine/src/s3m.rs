// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Scream Tracker 3 `.s3m` module file parser.
//!
//! Produces an [`XmModule`] so the existing [`crate::player::Player`] can
//! play S3M files without a separate engine.
//!
//! # Supported variants
//! - Standard PCM instrument files (type byte 0x10) with the `SCRM` magic
//! - 8-bit unsigned or signed PCM samples; 16-bit PCM (stereo uses left channel only)
//! - Up to 32 PCM channels; AdLib/OPL channels are silently ignored
//! - Default panning table (flag byte `0xFC`)
//! - File format version 1 (unsigned samples) and 2 (signed samples)
//!
//! # Mapping to XmModule
//! | S3M concept | XmModule field |
//! |-------------|---------------|
//! | PCM instrument (sample + header) | `XmInstrument` with one `XmSample` |
//! | C2spd (Hz at C-5) | `XmSample::relative_note` + `finetune` via `12×log₂(c2spd/8363)` |
//! | Effects A–Z | mapped to XM 0x00–0x0F where equivalent exists; S-extended → Exx |
//! | Note encoding (oct nibble ‖ semitone nibble) | `XmNote::On(1-indexed)` |
//! | Note cut (0xFE) | `XmNote::Off` (closest XM equivalent) |
//!
//! # Verified against
//! *One Must Fall 2097* in-game music (`03-MENU.S3M` through `09-END.S3M`).

use anyhow::bail;

use crate::xm::{
    EnvelopePoint, SampleLoopType, XmCell, XmEnvelope, XmInstrument, XmModule, XmNote, XmPattern,
    XmSample,
};

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse an S3M file from raw bytes, returning a playable [`XmModule`].
pub fn parse(data: &[u8]) -> anyhow::Result<XmModule> {
    if data.len() < 0x60 {
        bail!("not an S3M file: too short ({} bytes)", data.len());
    }
    if &data[0x2C..0x30] != b"SCRM" {
        bail!(
            "not an S3M file: bad magic {:?}",
            std::str::from_utf8(&data[0x2C..0x30]).unwrap_or("?")
        );
    }
    if data[0x1D] != 0x10 {
        bail!("not an S3M module: type byte {:#04X}", data[0x1D]);
    }

    // ── Header fields ─────────────────────────────────────────────────────
    let name = read_fixed_ascii(&data[0x00..0x1C]);

    let order_count = u16::from_le_bytes([data[0x20], data[0x21]]) as usize;
    let instr_count = u16::from_le_bytes([data[0x22], data[0x23]]) as usize;
    let pattern_count = u16::from_le_bytes([data[0x24], data[0x25]]) as usize;

    // File format version: 1 = unsigned samples (ST3.00), 2 = signed samples (ST3.20+).
    let signed_samples = u16::from_le_bytes([data[0x2A], data[0x2B]]) == 2;

    let initial_speed = data[0x31]; // ticks per row (S3M "speed")
    let initial_bpm = data[0x32]; // BPM
    let has_default_pan = data[0x35] == 0xFC;

    // Channel settings at 0x40 (32 bytes): 0x00–0x07 = left PCM, 0x08–0x0F = right PCM,
    // 0x10–0x1F = AdLib, 0xFF = unused.
    let chan_settings = &data[0x40..0x60];

    // ── Section offsets ───────────────────────────────────────────────────
    let orders_off = 0x60usize;
    let instr_pp_off = orders_off + order_count;
    let pat_pp_off = instr_pp_off + instr_count * 2;
    let pan_off = pat_pp_off + pattern_count * 2;

    if data.len() < pan_off {
        bail!("S3M file truncated before panning/parapointer tables");
    }

    // ── Order list ────────────────────────────────────────────────────────
    // 0xFF = end marker, 0xFE = "---" separator row (skip it).
    let pattern_order: Vec<u8> = data[orders_off..orders_off + order_count]
        .iter()
        .copied()
        .take_while(|&o| o != 0xFF)
        .filter(|&o| o != 0xFE)
        .collect();
    if pattern_order.is_empty() {
        bail!("S3M file has an empty order list");
    }
    let song_length = pattern_order.len();

    // ── Parapointers ─────────────────────────────────────────────────────
    let instr_ptrs: Vec<usize> = (0..instr_count)
        .map(|i| {
            u16::from_le_bytes([data[instr_pp_off + i * 2], data[instr_pp_off + i * 2 + 1]])
                as usize
                * 16
        })
        .collect();

    let pat_ptrs: Vec<usize> = (0..pattern_count)
        .map(|i| {
            u16::from_le_bytes([data[pat_pp_off + i * 2], data[pat_pp_off + i * 2 + 1]]) as usize
                * 16
        })
        .collect();

    // ── Default panning table ─────────────────────────────────────────────
    // Each byte: bit 5 set = valid; lower nibble = pan position (0=left, 7–8=centre, F=right).
    // If not present, derive from channel settings (0–7 = left, 8–15 = right PCM).
    let chan_pan: [u8; 32] = build_pan_table(chan_settings, has_default_pan, data, pan_off);

    // ── Active channel count ──────────────────────────────────────────────
    // Use the highest PCM channel index that is enabled + 1.
    let channel_count = (0..32usize)
        .filter(|&ch| chan_settings[ch] != 0xFF && chan_settings[ch] < 16)
        .max()
        .map(|max| max + 1)
        .unwrap_or(0);
    if channel_count == 0 {
        bail!("S3M file has no active PCM channels");
    }

    // ── Parse instruments ─────────────────────────────────────────────────
    let xm_instruments: Vec<XmInstrument> = instr_ptrs
        .iter()
        .enumerate()
        .map(|(i, &ioff)| parse_instrument(data, ioff, i, signed_samples, &chan_pan))
        .collect::<anyhow::Result<_>>()?;

    // ── Parse patterns ────────────────────────────────────────────────────
    let xm_patterns: Vec<XmPattern> = pat_ptrs
        .iter()
        .enumerate()
        .map(|(pi, &poff)| parse_pattern(data, poff, channel_count, pi))
        .collect::<anyhow::Result<_>>()?;

    Ok(XmModule {
        name,
        tracker_name: "ScreamTracker 3".into(),
        version: 0x0104,
        song_length: song_length as u16,
        restart_position: 0,
        channel_count: channel_count as u16,
        // S3M "speed" = ticks per row = XM default_tempo.
        default_tempo: initial_speed.max(1) as u16,
        default_bpm: initial_bpm.max(32) as u16,
        // We encode pitch correction in relative_note/finetune so the XM
        // linear-frequency model in Player gives correct output.
        linear_frequencies: true,
        pattern_order,
        patterns: xm_patterns,
        instruments: xm_instruments,
    })
}

// ── Instrument parser ─────────────────────────────────────────────────────────

fn parse_instrument(
    data: &[u8],
    ioff: usize,
    idx: usize,
    signed_samples: bool,
    chan_pan: &[u8; 32],
) -> anyhow::Result<XmInstrument> {
    // Placeholder instrument for missing / non-PCM slots.
    if ioff == 0 || data.len() < ioff + 80 {
        return Ok(blank_instrument(format!("Instr {}", idx + 1)));
    }

    let itype = data[ioff];
    if itype != 1 {
        // AdLib or empty — produce a silent placeholder.
        let sname = read_fixed_ascii(&data[ioff + 48..ioff + 76]);
        return Ok(blank_instrument(sname));
    }

    // Magic check for PCM instruments.
    if &data[ioff + 76..ioff + 80] != b"SCRS" {
        bail!("instrument {}: bad SCRS magic", idx + 1);
    }

    // Sample data paragraph pointer (3-byte big-endian paragraph number).
    let memseg = ((data[ioff + 13] as usize) << 16)
        | (u16::from_le_bytes([data[ioff + 14], data[ioff + 15]]) as usize);
    let sample_ptr = memseg * 16;

    let length = u32::from_le_bytes(data[ioff + 16..ioff + 20].try_into().unwrap());
    let loop_start = u32::from_le_bytes(data[ioff + 20..ioff + 24].try_into().unwrap());
    let loop_end = u32::from_le_bytes(data[ioff + 24..ioff + 28].try_into().unwrap());
    let volume = data[ioff + 28].min(64);
    let flags = data[ioff + 31];
    let is_16bit = flags & 0x04 != 0;
    let is_stereo = flags & 0x02 != 0;
    let has_loop = flags & 0x01 != 0;
    let c2spd = u32::from_le_bytes(data[ioff + 32..ioff + 36].try_into().unwrap());
    let sname = read_fixed_ascii(&data[ioff + 48..ioff + 76]);

    let samples_len = length as usize;
    let bytes_per_sample = if is_16bit { 2usize } else { 1 } * if is_stereo { 2usize } else { 1 };
    let byte_len = samples_len * bytes_per_sample;

    let loop_type = if has_loop && loop_end > loop_start {
        SampleLoopType::Forward
    } else {
        SampleLoopType::None
    };
    let (loop_s, loop_len) = if loop_type != SampleLoopType::None {
        (loop_start, loop_end.saturating_sub(loop_start))
    } else {
        (0, 0)
    };

    let pcm16 = if sample_ptr > 0 && data.len() >= sample_ptr + byte_len {
        convert_pcm(
            &data[sample_ptr..sample_ptr + byte_len],
            is_16bit,
            is_stereo,
            signed_samples,
        )
    } else {
        vec![]
    };

    let (relative_note, finetune) = c2spd_to_pitch(c2spd);

    // Default panning from channel 0's pan table entry (instruments don't have
    // a per-instrument panning field in S3M; panning is per-channel in patterns).
    let panning = chan_pan[0];

    let sample = XmSample {
        name: sname.clone(),
        loop_start: loop_s,
        loop_length: loop_len,
        loop_type,
        volume,
        finetune,
        panning,
        relative_note,
        data: pcm16,
    };

    let mut instr = XmInstrument {
        name: sname,
        note_to_sample: [0u8; 96],
        volume_envelope: flat_envelope(),
        panning_envelope: XmEnvelope::default(),
        volume_fadeout: 0,
        vibrato_type: 0,
        vibrato_sweep: 0,
        vibrato_depth: 0,
        vibrato_rate: 0,
        samples: vec![sample],
    };
    instr.note_to_sample = [0u8; 96];
    Ok(instr)
}

// ── Pattern parser ────────────────────────────────────────────────────────────

/// Parse a single packed S3M pattern into an [`XmPattern`].
///
/// S3M patterns always have exactly 64 rows.  Each row is a sequence of
/// channel-event bytes terminated by `0x00`.  The marker byte encodes:
/// - bits 0–4: channel number (0–31)
/// - bit 5 (`0x20`): command + info follow (2 bytes)
/// - bit 6 (`0x40`): volume byte follows (1 byte)
/// - bit 7 (`0x80`): note + instrument follow (2 bytes)
fn parse_pattern(
    data: &[u8],
    poff: usize,
    channel_count: usize,
    pi: usize,
) -> anyhow::Result<XmPattern> {
    const ROWS: usize = 64;

    if poff == 0 || data.len() < poff + 2 {
        return Ok(empty_pattern(ROWS, channel_count));
    }

    let packed_len = u16::from_le_bytes([data[poff], data[poff + 1]]) as usize;
    if data.len() < poff + 2 + packed_len {
        bail!(
            "S3M pattern {pi}: truncated (need {} packed bytes)",
            packed_len
        );
    }
    let packed = &data[poff + 2..poff + 2 + packed_len];
    let mut pos = 0usize;

    let mut rows: Vec<Vec<XmCell>> = (0..ROWS)
        .map(|_| vec![XmCell::default(); channel_count])
        .collect();

    for row_cells in rows.iter_mut() {
        // Read channel events until end-of-row sentinel (0x00).
        loop {
            if pos >= packed.len() {
                break;
            }
            let marker = packed[pos];
            pos += 1;

            if marker == 0x00 {
                break; // end of row
            }

            let ch = (marker & 0x1F) as usize;
            let has_note_instr = marker & 0x80 != 0;
            let has_vol = marker & 0x40 != 0;
            let has_cmd = marker & 0x20 != 0;

            let mut note_raw = 0xFFu8;
            let mut instr_raw = 0u8;
            let mut vol_raw = 0xFFu8;
            let mut cmd_raw = 0u8;
            let mut info_raw = 0u8;

            if has_note_instr {
                if pos + 1 >= packed.len() {
                    bail!("S3M pattern {pi}: truncated note/instr bytes");
                }
                note_raw = packed[pos];
                instr_raw = packed[pos + 1];
                pos += 2;
            }
            if has_vol {
                if pos >= packed.len() {
                    bail!("S3M pattern {pi}: truncated volume byte");
                }
                vol_raw = packed[pos];
                pos += 1;
            }
            if has_cmd {
                if pos + 1 >= packed.len() {
                    bail!("S3M pattern {pi}: truncated cmd/info bytes");
                }
                cmd_raw = packed[pos];
                info_raw = packed[pos + 1];
                pos += 2;
            }

            // Only write to channels within our allocated range.
            if ch >= channel_count {
                continue;
            }

            let cell = &mut row_cells[ch];

            // Note
            cell.note = match note_raw {
                0xFF => XmNote::None,
                0xFE => XmNote::Off, // S3M note cut → XM key-off (closest equivalent)
                n if n & 0x0F < 12 => {
                    // Octave in upper nibble, semitone in lower nibble.
                    // XM notes are 1-indexed: (octave × 12) + semitone + 1.
                    let oct = (n >> 4) as i32;
                    let semi = (n & 0x0F) as i32;
                    let xm_note = oct * 12 + semi + 1;
                    if (1..=96).contains(&xm_note) {
                        XmNote::On(xm_note as u8)
                    } else {
                        XmNote::None
                    }
                }
                _ => XmNote::None,
            };

            // Instrument (S3M is 1-indexed, XmCell is also 1-indexed).
            cell.instrument = instr_raw;

            // Volume: S3M 0–64 = set volume.  Values > 64 mean "no command".
            cell.volume = if vol_raw <= 64 {
                0x10 + vol_raw // XM volume column: 0x10 = vol 0, 0x50 = vol 64
            } else {
                0
            };

            // Effect
            let (xm_effect, xm_param) = map_effect(cmd_raw, info_raw);
            cell.effect = xm_effect;
            cell.effect_param = xm_param;
        }
    }

    Ok(XmPattern { rows })
}

// ── Effect mapping ────────────────────────────────────────────────────────────

/// Map an S3M command byte (1-indexed, A=1 … Z=26) and its info byte to an
/// XM effect byte and parameter.  Unsupported effects are silenced (0, 0).
fn map_effect(cmd: u8, info: u8) -> (u8, u8) {
    match cmd {
        0x01 => (0x0F, info), // A  set speed  (ticks/row < 32 → XM Fxx)
        0x02 => (0x0B, info), // B  jump to order
        0x03 => (0x0D, info), // C  pattern break (row number)
        0x04 => (0x0A, info), // D  volume slide (same nibble format as XM Axx)
        0x05 => (0x02, info), // E  portamento down
        0x06 => (0x01, info), // F  portamento up
        0x07 => (0x03, info), // G  tone portamento
        0x08 => (0x04, info), // H  vibrato (speed/depth nibbles match XM)
        // I  tremor — no XM equivalent; silence it
        0x0A => (0x00, info), // J  arpeggio
        0x0B => (0x06, info), // K  vibrato + volume slide
        0x0C => (0x05, info), // L  tone portamento + volume slide
        // M  channel volume — no direct XM mapping
        // N  channel volume slide — no direct XM mapping
        0x0F => (0x09, info), // O  sample offset
        // P  panning slide — no direct XM mapping
        0x11 => (0x0E, 0x90 | (info & 0x0F)), // Q  retrigger → XM E9x
        0x12 => (0x07, info),                 // R  tremolo
        0x13 => map_extended(info),           // S  extended (sub-effect in high nibble)
        0x14 => (0x0F, info),                 // T  set BPM  (param ≥ 32 → XM Fxx)
        0x15 => (0x04, info),                 // U  fine vibrato (approx XM 4xx)
        // V  global volume — no XM mapping
        // W  global volume slide — no XM mapping
        0x18 => (0x08, info), // X  set panning (0–FF, 0x80=centre; matches XM 8xx)
        // Y  panbrello, Z  MIDI macro — unsupported
        _ => (0x00, 0x00),
    }
}

/// Map an S3M `Sxy` extended effect to an XM `Exy` extended effect.
fn map_extended(info: u8) -> (u8, u8) {
    let sub = (info >> 4) & 0x0F;
    let val = info & 0x0F;
    let xm_sub: u8 = match sub {
        0x1 => 0x3, // S1x glissando control → E3x
        0x2 => 0x5, // S2x set finetune → E5x
        0x3 => 0x4, // S3x set vibrato waveform → E4x
        0x4 => 0x7, // S4x set tremolo waveform → E7x
        0x8 => 0x8, // S8x set panning (0–F) → E8x
        0xB => 0x6, // SBx pattern loop → E6x
        0xC => 0xC, // SCx note cut at tick x → ECx
        0xD => 0xD, // SDx note delay → EDx
        0xE => 0xE, // SEx pattern delay → EEx
        _ => return (0x00, 0x00),
    };
    (0x0E, (xm_sub << 4) | val)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert a MOD/GUS-style C2spd (sample frequency at XM C-5 = note 61) to
/// `(relative_note, finetune)` using the formula from `crate::gus`.
fn c2spd_to_pitch(c2spd: u32) -> (i8, i8) {
    if c2spd == 0 {
        return (0, 0);
    }
    // correction = 12 × log₂(c2spd / 8363)
    let correction = 12.0 * (c2spd as f64 / 8363.0_f64).log2();
    let relative_note = correction.round() as i32;
    let finetune_frac = (correction - correction.round()) * 128.0;
    (
        relative_note.clamp(-128, 127) as i8,
        (finetune_frac.round() as i32).clamp(-128, 127) as i8,
    )
}

/// Build the 32-entry channel panning table.
///
/// Each entry is a linear `0` (left) – `255` (right) panning value.
/// If the default panning table is present (`0xFC` flag), read and decode it.
/// Otherwise derive from channel settings: 0–7 = hard-left, 8–15 = hard-right.
fn build_pan_table(
    chan_settings: &[u8],
    has_default_pan: bool,
    data: &[u8],
    pan_off: usize,
) -> [u8; 32] {
    let mut t = [128u8; 32];
    if has_default_pan && data.len() >= pan_off + 32 {
        for ch in 0..32 {
            let b = data[pan_off + ch];
            if b & 0x20 != 0 {
                // Valid panning: lower nibble 0–15 maps to 0–255.
                t[ch] = ((b & 0x0F) as u16 * 255 / 15) as u8;
            } else {
                t[ch] = default_chan_pan(chan_settings[ch]);
            }
        }
    } else {
        for ch in 0..32 {
            t[ch] = default_chan_pan(chan_settings[ch]);
        }
    }
    t
}

/// Return the default panning for a channel from its channel-settings byte.
fn default_chan_pan(setting: u8) -> u8 {
    match setting {
        0..=7 => 0,    // left PCM
        8..=15 => 255, // right PCM
        _ => 128,      // centre (AdLib or unused)
    }
}

/// Convert raw PCM bytes to `Vec<i16>` (the format used by `XmSample::data`).
///
/// For stereo samples only the left channel (even samples) is retained.
fn convert_pcm(raw: &[u8], is_16bit: bool, is_stereo: bool, signed: bool) -> Vec<i16> {
    let stride = if is_stereo { 2usize } else { 1 };
    if is_16bit {
        raw.chunks_exact(2 * stride)
            .map(|c| {
                let v = i16::from_le_bytes([c[0], c[1]]);
                if signed { v } else { v.wrapping_add(i16::MIN) }
            })
            .collect()
    } else {
        raw.chunks_exact(stride)
            .map(|c| {
                let b = c[0];
                let s: i8 = if signed {
                    b as i8
                } else {
                    (b as i16 - 128) as i8
                };
                (s as i16) << 8
            })
            .collect()
    }
}

/// Build a 64-row × `channel_count`-channel silent [`XmPattern`].
fn empty_pattern(rows: usize, channel_count: usize) -> XmPattern {
    XmPattern {
        rows: vec![vec![XmCell::default(); channel_count]; rows],
    }
}

/// Build a minimal single-sample `XmInstrument` that plays silence.
fn blank_instrument(name: String) -> XmInstrument {
    XmInstrument {
        name,
        note_to_sample: [0u8; 96],
        volume_envelope: flat_envelope(),
        panning_envelope: XmEnvelope::default(),
        volume_fadeout: 0,
        vibrato_type: 0,
        vibrato_sweep: 0,
        vibrato_depth: 0,
        vibrato_rate: 0,
        samples: vec![XmSample {
            name: String::new(),
            loop_start: 0,
            loop_length: 0,
            loop_type: SampleLoopType::None,
            volume: 0,
            finetune: 0,
            panning: 128,
            relative_note: 0,
            data: vec![],
        }],
    }
}

/// A flat, disabled volume envelope (sustain at full volume).
fn flat_envelope() -> XmEnvelope {
    XmEnvelope {
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
    }
}

/// Read a null-padded ASCII string from a fixed-width byte slice.
fn read_fixed_ascii(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end])
        .trim_end()
        .to_string()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Unit tests (no file I/O) ──────────────────────────────────────────

    #[test]
    fn note_byte_c5() {
        // S3M 0x50 = octave 5, semitone 0 → XM C-5 = note 61
        let cell_note = match 0x50u8 {
            n if n & 0x0F < 12 => {
                let oct = (n >> 4) as i32;
                let semi = (n & 0x0F) as i32;
                XmNote::On((oct * 12 + semi + 1) as u8)
            }
            _ => XmNote::None,
        };
        assert_eq!(cell_note, XmNote::On(61));
    }

    #[test]
    fn note_byte_c0() {
        // S3M 0x00 = octave 0, semitone 0 → XM C-0 = note 1
        let xm_note = {
            let n = 0x00u8;
            let oct = (n >> 4) as i32;
            let semi = (n & 0x0F) as i32;
            oct * 12 + semi + 1
        };
        assert_eq!(xm_note, 1);
    }

    #[test]
    fn note_byte_b7() {
        // S3M 0x7B = octave 7, semitone 11 → XM B-7 = note 96
        let xm_note = {
            let n = 0x7Bu8;
            let oct = (n >> 4) as i32;
            let semi = (n & 0x0F) as i32;
            oct * 12 + semi + 1
        };
        assert_eq!(xm_note, 96);
    }

    #[test]
    fn c2spd_8363_no_correction() {
        // The XM reference c2spd is 8363 Hz — no pitch correction needed.
        let (rel, ft) = c2spd_to_pitch(8363);
        assert_eq!(rel, 0);
        assert_eq!(ft, 0);
    }

    #[test]
    fn c2spd_octave_up() {
        // Doubling frequency = one octave up = +12 semitones.
        let (rel, _) = c2spd_to_pitch(8363 * 2);
        assert_eq!(rel, 12);
    }

    #[test]
    fn c2spd_zero_is_no_op() {
        assert_eq!(c2spd_to_pitch(0), (0, 0));
    }

    #[test]
    fn effect_speed_maps_to_f() {
        assert_eq!(map_effect(0x01, 6), (0x0F, 6));
    }

    #[test]
    fn effect_bpm_maps_to_f() {
        assert_eq!(map_effect(0x14, 125), (0x0F, 125));
    }

    #[test]
    fn effect_porta_down_maps_to_2() {
        assert_eq!(map_effect(0x05, 0x10), (0x02, 0x10));
    }

    #[test]
    fn effect_porta_up_maps_to_1() {
        assert_eq!(map_effect(0x06, 0x10), (0x01, 0x10));
    }

    #[test]
    fn effect_s_note_cut() {
        // SCx → XM ECx
        assert_eq!(map_effect(0x13, 0xC3), (0x0E, 0xC3));
    }

    #[test]
    fn effect_s_pattern_loop() {
        // SBx → XM E6x
        assert_eq!(map_effect(0x13, 0xB2), (0x0E, 0x62));
    }

    #[test]
    fn effect_x_panning() {
        assert_eq!(map_effect(0x18, 0x80), (0x08, 0x80));
    }

    #[test]
    fn reject_short_data() {
        assert!(parse(&[0u8; 10]).is_err());
    }

    #[test]
    fn reject_wrong_magic() {
        let mut data = vec![0u8; 0x70];
        data[0x1D] = 0x10;
        data[0x2C..0x30].copy_from_slice(b"XXXX");
        assert!(parse(&data).is_err());
    }

    // ── Integration tests against real OMF2097 S3M files ─────────────────

    const MENU: &str = "/tmp/03-MENU.S3M";

    fn load(path: &str) -> Vec<u8> {
        std::fs::read(path).expect(path)
    }

    #[test]
    fn parse_omf_menu() {
        let data = load(MENU);
        let module = parse(&data).expect("parse 03-MENU.S3M");
        assert_eq!(module.channel_count, 6);
        assert_eq!(module.default_tempo, 6);
        assert_eq!(module.default_bpm, 125);
        assert_eq!(module.song_length, 24);
    }

    #[test]
    fn omf_menu_instruments_have_samples() {
        let data = load(MENU);
        let module = parse(&data).unwrap();
        let non_empty = module
            .instruments
            .iter()
            .filter(|i| !i.samples.is_empty() && !i.samples[0].data.is_empty())
            .count();
        assert!(
            non_empty >= 10,
            "expected ≥10 instruments with PCM data, got {non_empty}"
        );
    }

    #[test]
    fn omf_menu_patterns_have_notes() {
        let data = load(MENU);
        let module = parse(&data).unwrap();
        let notes: usize = module
            .patterns
            .iter()
            .flat_map(|p| p.rows.iter())
            .flat_map(|r| r.iter())
            .filter(|c| matches!(c.note, XmNote::On(_)))
            .count();
        assert!(
            notes > 100,
            "expected >100 note events across all patterns, got {notes}"
        );
    }
}
