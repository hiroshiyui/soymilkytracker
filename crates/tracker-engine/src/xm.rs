// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! XM (Extended Module) file parser.
//!
//! Implements the FastTracker II XM specification (version 0x0104).
//!
//! # File layout
//! ```text
//! [preamble: 60 bytes]   magic + module name + tracker name + version
//! [header: header_size]  song parameters + 256-byte pattern order table
//! [patterns × N]         each: header + packed note data
//! [instruments × N]      each: header + sample headers + sample data
//! ```
//!
//! Reference: <https://github.com/milkytracker/MilkyTracker/blob/master/resources/reference/xm-form.txt>

use std::io::{Cursor, Read, Seek, SeekFrom};

use anyhow::bail;

// ── Public types ─────────────────────────────────────────────────────────────

/// A fully-parsed XM module.
#[derive(Debug, Clone)]
pub struct XmModule {
    /// Module title (up to 20 characters).
    pub name: String,
    /// Tracker software that created this file.
    pub tracker_name: String,
    /// XM format version encoded in the file (typically 0x0104).
    pub version: u16,
    /// Number of entries used in the pattern order table.
    pub song_length: u16,
    /// Pattern order index to restart from when looping.
    pub restart_position: u16,
    /// Number of channels (2–32).
    pub channel_count: u16,
    /// Default ticks per row (speed).
    pub default_tempo: u16,
    /// Default beats per minute.
    pub default_bpm: u16,
    /// `true` → linear (FastTracker II) frequency table; `false` → Amiga table.
    pub linear_frequencies: bool,
    /// Pattern play order; each value is an index into `patterns`.
    pub pattern_order: Vec<u8>,
    /// All patterns in file order.
    pub patterns: Vec<XmPattern>,
    /// All instruments, 1-indexed (`instruments[0]` = instrument 1).
    pub instruments: Vec<XmInstrument>,
}

/// One pattern: a rectangular grid of cells addressed as `rows[row][channel]`.
#[derive(Debug, Clone)]
pub struct XmPattern {
    pub rows: Vec<Vec<XmCell>>,
}

/// A single cell in the pattern editor grid.
#[derive(Debug, Clone, Default)]
pub struct XmCell {
    /// Note (None if the column is empty).
    pub note: XmNote,
    /// 1-based instrument index, 0 = no instrument.
    pub instrument: u8,
    /// Volume column byte (`0x10`–`0x50` = set volume 0–64; other values
    /// encode special commands such as slide up/down, vibrato, etc.).
    pub volume: u8,
    /// Effect type nibble (0x00–0x24).
    pub effect: u8,
    /// Effect parameter byte.
    pub effect_param: u8,
}

/// The note field of an [`XmCell`].
#[derive(Debug, Clone, Default, PartialEq)]
pub enum XmNote {
    /// No note event.
    #[default]
    None,
    /// Note on — value 1 (C-0) through 96 (B-7).
    On(u8),
    /// Key-off event (raw value 97).
    Off,
}

/// One instrument, which owns zero or more samples.
#[derive(Debug, Clone)]
pub struct XmInstrument {
    pub name: String,
    /// Maps each of the 96 semitones (C-0 … B-7) to a 0-based sample index.
    pub note_to_sample: [u8; 96],
    pub volume_envelope: XmEnvelope,
    pub panning_envelope: XmEnvelope,
    /// Volume fade-out speed per tick; 0 = no fade-out.
    pub volume_fadeout: u16,
    pub vibrato_type: u8,
    pub vibrato_sweep: u8,
    pub vibrato_depth: u8,
    pub vibrato_rate: u8,
    pub samples: Vec<XmSample>,
}

/// Volume or panning envelope with up to 12 breakpoints.
#[derive(Debug, Clone, Default)]
pub struct XmEnvelope {
    /// Up to 12 (tick, value) breakpoints.
    pub points: Vec<EnvelopePoint>,
    pub sustain_point: u8,
    pub loop_start: u8,
    pub loop_end: u8,
    /// Envelope is active.
    pub enabled: bool,
    /// Hold at sustain point until note-off.
    pub sustain: bool,
    /// Loop between `loop_start` and `loop_end`.
    pub looped: bool,
}

/// One breakpoint of an [`XmEnvelope`]: time in ticks + value (0–64).
#[derive(Debug, Clone, Copy)]
pub struct EnvelopePoint {
    pub tick: u16,
    pub value: u16,
}

/// A single sample within an instrument.
#[derive(Debug, Clone)]
pub struct XmSample {
    pub name: String,
    /// Loop start in sample frames.
    pub loop_start: u32,
    /// Loop length in sample frames.
    pub loop_length: u32,
    pub loop_type: SampleLoopType,
    /// Linear volume 0–64.
    pub volume: u8,
    /// Fine-tuning: −128 … +127 (units of 1/128 semitone).
    pub finetune: i8,
    /// Panning 0–255 (128 = centre).
    pub panning: u8,
    /// Semitone transposition relative to C-5.
    pub relative_note: i8,
    /// Decoded, delta-decompressed sample data, normalised to 16-bit.
    ///
    /// 8-bit source samples are left-shifted by 8, so the full i16 range
    /// is always used regardless of the original bit depth.
    pub data: Vec<i16>,
}

/// Loop mode for a sample.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SampleLoopType {
    #[default]
    None,
    Forward,
    PingPong,
}

// ── Public entry point ───────────────────────────────────────────────────────

/// Parse an XM module from raw bytes.
///
/// # Errors
/// Returns an error if the data is not a valid XM file, the version is
/// unsupported, or the data is truncated.
pub fn parse(data: &[u8]) -> anyhow::Result<XmModule> {
    parse_inner(&mut Cursor::new(data))
}

// ── Parser internals ─────────────────────────────────────────────────────────

fn parse_inner(r: &mut Cursor<&[u8]>) -> anyhow::Result<XmModule> {
    // ── Preamble (60 bytes) ───────────────────────────────────────────────
    let mut magic = [0u8; 17];
    r.read_exact(&mut magic)?;
    if &magic != b"Extended Module: " {
        bail!("not an XM file: invalid magic bytes");
    }
    let name = read_string(r, 20)?;
    if read_u8(r)? != 0x1A {
        bail!("not an XM file: missing 0x1A marker");
    }
    let tracker_name = read_string(r, 20)?;
    let version = read_u16_le(r)?;
    if !(0x0103..=0x0104).contains(&version) {
        bail!("unsupported XM version 0x{version:04X} (expected 0x0103 or 0x0104)");
    }

    // ── Song header ───────────────────────────────────────────────────────
    let header_start = r.position(); // = 60
    let header_size = read_u32_le(r)?;
    let song_length = read_u16_le(r)?;
    let restart_position = read_u16_le(r)?;
    let channel_count = read_u16_le(r)?;
    let pattern_count = read_u16_le(r)?;
    let instrument_count = read_u16_le(r)?;
    let flags = read_u16_le(r)?;
    let linear_frequencies = (flags & 1) != 0;
    let default_tempo = read_u16_le(r)?;
    let default_bpm = read_u16_le(r)?;

    // Pattern order table occupies the remainder of the header.
    let header_end = header_start + header_size as u64;
    let order_bytes = header_end.saturating_sub(r.position()) as usize;
    let order_bytes = order_bytes.min(256);
    let mut order_buf = vec![0u8; order_bytes];
    r.read_exact(&mut order_buf)?;
    let pattern_order = order_buf[..song_length.min(order_bytes as u16) as usize].to_vec();

    // Seek past the full header in case it was padded.
    r.seek(SeekFrom::Start(header_end))?;

    // ── Patterns ──────────────────────────────────────────────────────────
    let mut patterns = Vec::with_capacity(pattern_count as usize);
    for idx in 0..pattern_count {
        patterns.push(
            parse_pattern(r, channel_count as usize)
                .map_err(|e| anyhow::anyhow!("pattern {idx}: {e}"))?,
        );
    }

    // ── Instruments ───────────────────────────────────────────────────────
    let mut instruments = Vec::with_capacity(instrument_count as usize);
    for idx in 0..instrument_count {
        instruments
            .push(parse_instrument(r).map_err(|e| anyhow::anyhow!("instrument {}: {e}", idx + 1))?);
    }

    Ok(XmModule {
        name,
        tracker_name,
        version,
        song_length,
        restart_position,
        channel_count,
        default_tempo,
        default_bpm,
        linear_frequencies,
        pattern_order,
        patterns,
        instruments,
    })
}

// ── Pattern ──────────────────────────────────────────────────────────────────

fn parse_pattern(r: &mut Cursor<&[u8]>, channels: usize) -> anyhow::Result<XmPattern> {
    let pattern_start = r.position();
    let header_len = read_u32_le(r)?;
    let _packing_type = read_u8(r)?; // always 0
    let num_rows = read_u16_le(r)? as usize;
    let packed_size = read_u16_le(r)? as usize;

    // Seek to the end of the pattern header (may be >9 bytes in some variants).
    r.seek(SeekFrom::Start(pattern_start + header_len as u64))?;

    let rows = if packed_size == 0 {
        // Empty pattern: all cells default to silent.
        vec![vec![XmCell::default(); channels]; num_rows]
    } else {
        let mut packed = vec![0u8; packed_size];
        r.read_exact(&mut packed)?;
        unpack_pattern(&packed, num_rows, channels)?
    };

    Ok(XmPattern { rows })
}

/// Decode the run-length-like compression used for pattern data.
///
/// Each cell can be stored as 5 raw bytes or as a flag byte + up to 5
/// conditional bytes.  A flag byte has bit 7 set; bits 0–4 indicate
/// which of (note, instrument, volume, effect, effect-param) follow.
fn unpack_pattern(
    packed: &[u8],
    num_rows: usize,
    channels: usize,
) -> anyhow::Result<Vec<Vec<XmCell>>> {
    let mut rows = vec![vec![XmCell::default(); channels]; num_rows];
    let mut pos = 0;

    for (row, row_cells) in rows.iter_mut().enumerate() {
        for (ch, cell) in row_cells.iter_mut().enumerate() {
            if pos >= packed.len() {
                break;
            }
            let first = packed[pos];
            pos += 1;

            let (raw_note, raw_inst, raw_vol, raw_fx, raw_fxp);
            if first & 0x80 != 0 {
                // Compressed: first byte is a bitmask.
                let f = first;
                macro_rules! cond_read {
                    ($bit:expr) => {
                        if f & $bit != 0 {
                            if pos >= packed.len() {
                                bail!("truncated pattern data at row {row} ch {ch}");
                            }
                            let v = packed[pos];
                            pos += 1;
                            v
                        } else {
                            0
                        }
                    };
                }
                raw_note = cond_read!(0x01);
                raw_inst = cond_read!(0x02);
                raw_vol = cond_read!(0x04);
                raw_fx = cond_read!(0x08);
                raw_fxp = cond_read!(0x10);
            } else {
                // Uncompressed: byte already is the note; 4 more bytes follow.
                raw_note = first;
                if pos + 3 >= packed.len() {
                    bail!("truncated pattern data at row {row} ch {ch}");
                }
                raw_inst = packed[pos];
                raw_vol = packed[pos + 1];
                raw_fx = packed[pos + 2];
                raw_fxp = packed[pos + 3];
                pos += 4;
            }

            *cell = XmCell {
                note: match raw_note {
                    0 => XmNote::None,
                    97 => XmNote::Off,
                    n => XmNote::On(n),
                },
                instrument: raw_inst,
                volume: raw_vol,
                effect: raw_fx,
                effect_param: raw_fxp,
            };
        }
    }
    Ok(rows)
}

// ── Instrument ───────────────────────────────────────────────────────────────

fn parse_instrument(r: &mut Cursor<&[u8]>) -> anyhow::Result<XmInstrument> {
    let instr_start = r.position();
    let instr_header_size = read_u32_le(r)?;
    let name = read_string(r, 22)?;
    let _instr_type = read_u8(r)?;
    let num_samples = read_u16_le(r)?;

    // ── Extended instrument header (only present when num_samples > 0) ────
    let (
        note_to_sample,
        volume_envelope,
        panning_envelope,
        volume_fadeout,
        vibrato_type,
        vibrato_sweep,
        vibrato_depth,
        vibrato_rate,
        sample_header_size,
    );

    if num_samples > 0 {
        sample_header_size = read_u32_le(r)? as u64;

        let mut nts = [0u8; 96];
        r.read_exact(&mut nts)?;
        note_to_sample = nts;

        // Volume and panning envelopes: 12 × (tick: u16, value: u16) each.
        let mut vol_raw = [0u16; 24];
        for v in vol_raw.iter_mut() {
            *v = read_u16_le(r)?;
        }
        let mut pan_raw = [0u16; 24];
        for v in pan_raw.iter_mut() {
            *v = read_u16_le(r)?;
        }

        let vol_count = read_u8(r)? as usize;
        let pan_count = read_u8(r)? as usize;
        let vol_sustain = read_u8(r)?;
        let vol_loop_start = read_u8(r)?;
        let vol_loop_end = read_u8(r)?;
        let pan_sustain = read_u8(r)?;
        let pan_loop_start = read_u8(r)?;
        let pan_loop_end = read_u8(r)?;
        let vol_flags = read_u8(r)?;
        let pan_flags = read_u8(r)?;
        vibrato_type = read_u8(r)?;
        vibrato_sweep = read_u8(r)?;
        vibrato_depth = read_u8(r)?;
        vibrato_rate = read_u8(r)?;
        volume_fadeout = read_u16_le(r)?;
        let _reserved = read_u16_le(r)?;

        volume_envelope = make_envelope(
            &vol_raw,
            vol_count.min(12),
            vol_sustain,
            vol_loop_start,
            vol_loop_end,
            vol_flags,
        );
        panning_envelope = make_envelope(
            &pan_raw,
            pan_count.min(12),
            pan_sustain,
            pan_loop_start,
            pan_loop_end,
            pan_flags,
        );
    } else {
        sample_header_size = 0;
        note_to_sample = [0u8; 96];
        volume_envelope = XmEnvelope::default();
        panning_envelope = XmEnvelope::default();
        volume_fadeout = 0;
        vibrato_type = 0;
        vibrato_sweep = 0;
        vibrato_depth = 0;
        vibrato_rate = 0;
    }

    // Seek past the full instrument header (may be padded).
    r.seek(SeekFrom::Start(instr_start + instr_header_size as u64))?;

    // ── Sample headers ────────────────────────────────────────────────────
    // All sample headers are read before any sample data.
    let mut raw_headers = Vec::with_capacity(num_samples as usize);
    for idx in 0..num_samples as u64 {
        let hdr_start = r.position();
        let hdr = read_sample_header(r).map_err(|e| anyhow::anyhow!("sample {idx} header: {e}"))?;
        raw_headers.push(hdr);
        // Seek past padding (sample_header_size may be >40).
        if sample_header_size > 0 {
            r.seek(SeekFrom::Start(hdr_start + sample_header_size))?;
        }
    }

    // ── Sample data ───────────────────────────────────────────────────────
    let mut samples = Vec::with_capacity(raw_headers.len());
    for (idx, hdr) in raw_headers.into_iter().enumerate() {
        let data = read_sample_data(r, hdr.length_bytes, hdr.is_16bit)
            .map_err(|e| anyhow::anyhow!("sample {idx} data: {e}"))?;
        // Convert byte-based loop offsets to sample-frame offsets.
        let (loop_start, loop_length) = if hdr.is_16bit {
            (hdr.loop_start_bytes / 2, hdr.loop_length_bytes / 2)
        } else {
            (hdr.loop_start_bytes, hdr.loop_length_bytes)
        };
        samples.push(XmSample {
            name: hdr.name,
            loop_start,
            loop_length,
            loop_type: hdr.loop_type,
            volume: hdr.volume,
            finetune: hdr.finetune,
            panning: hdr.panning,
            relative_note: hdr.relative_note,
            data,
        });
    }

    Ok(XmInstrument {
        name,
        note_to_sample,
        volume_envelope,
        panning_envelope,
        volume_fadeout,
        vibrato_type,
        vibrato_sweep,
        vibrato_depth,
        vibrato_rate,
        samples,
    })
}

struct RawSampleHeader {
    length_bytes: u32,
    loop_start_bytes: u32,
    loop_length_bytes: u32,
    volume: u8,
    finetune: i8,
    panning: u8,
    relative_note: i8,
    loop_type: SampleLoopType,
    is_16bit: bool,
    name: String,
}

fn read_sample_header(r: &mut Cursor<&[u8]>) -> anyhow::Result<RawSampleHeader> {
    let length_bytes = read_u32_le(r)?;
    let loop_start_bytes = read_u32_le(r)?;
    let loop_length_bytes = read_u32_le(r)?;
    let volume = read_u8(r)?;
    let finetune = read_u8(r)? as i8;
    let type_byte = read_u8(r)?;
    let panning = read_u8(r)?;
    let relative_note = read_u8(r)? as i8;
    let _reserved = read_u8(r)?;
    let name = read_string(r, 22)?;

    let loop_type = match type_byte & 0x03 {
        1 => SampleLoopType::Forward,
        2 => SampleLoopType::PingPong,
        _ => SampleLoopType::None,
    };
    let is_16bit = (type_byte & 0x10) != 0;

    Ok(RawSampleHeader {
        length_bytes,
        loop_start_bytes,
        loop_length_bytes,
        volume,
        finetune,
        panning,
        relative_note,
        loop_type,
        is_16bit,
        name,
    })
}

/// Read and delta-decompress raw sample bytes, producing `Vec<i16>`.
///
/// XM sample data uses *signed delta encoding*: each byte/word is the
/// signed difference from the previous value, starting from zero.
/// 8-bit samples are left-shifted to fill the i16 range.
fn read_sample_data(
    r: &mut Cursor<&[u8]>,
    length_bytes: u32,
    is_16bit: bool,
) -> anyhow::Result<Vec<i16>> {
    if length_bytes == 0 {
        return Ok(Vec::new());
    }
    if is_16bit {
        let n = (length_bytes / 2) as usize;
        let mut data = Vec::with_capacity(n);
        for _ in 0..n {
            data.push(read_i16_le(r)?);
        }
        delta_decode_i16(&mut data);
        Ok(data)
    } else {
        let n = length_bytes as usize;
        let mut raw = Vec::with_capacity(n);
        for _ in 0..n {
            raw.push(read_u8(r)? as i8);
        }
        delta_decode_i8(&mut raw);
        Ok(raw.iter().map(|&s| (s as i16) << 8).collect())
    }
}

// ── Delta decoders ───────────────────────────────────────────────────────────

fn delta_decode_i8(data: &mut [i8]) {
    let mut acc: i8 = 0;
    for s in data.iter_mut() {
        acc = acc.wrapping_add(*s);
        *s = acc;
    }
}

fn delta_decode_i16(data: &mut [i16]) {
    let mut acc: i16 = 0;
    for s in data.iter_mut() {
        acc = acc.wrapping_add(*s);
        *s = acc;
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_envelope(
    raw: &[u16; 24],
    count: usize,
    sustain: u8,
    loop_start: u8,
    loop_end: u8,
    flags: u8,
) -> XmEnvelope {
    let points = (0..count)
        .map(|i| EnvelopePoint {
            tick: raw[i * 2],
            value: raw[i * 2 + 1],
        })
        .collect();
    XmEnvelope {
        points,
        sustain_point: sustain,
        loop_start,
        loop_end,
        enabled: (flags & 0x01) != 0,
        sustain: (flags & 0x02) != 0,
        looped: (flags & 0x04) != 0,
    }
}

// ── Byte-level I/O ───────────────────────────────────────────────────────────

fn read_u8(r: &mut Cursor<&[u8]>) -> anyhow::Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    Ok(b[0])
}

fn read_u16_le(r: &mut Cursor<&[u8]>) -> anyhow::Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(u16::from_le_bytes(b))
}

fn read_u32_le(r: &mut Cursor<&[u8]>) -> anyhow::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_i16_le(r: &mut Cursor<&[u8]>) -> anyhow::Result<i16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(i16::from_le_bytes(b))
}

/// Read `len` bytes and return a trimmed UTF-8 string (null bytes stripped).
fn read_string(r: &mut Cursor<&[u8]>, len: usize) -> anyhow::Result<String> {
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    let trimmed: Vec<u8> = buf.into_iter().take_while(|&b| b != 0).collect();
    Ok(String::from_utf8_lossy(&trimmed).trim_end().to_string())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Unit tests for internal helpers ──────────────────────────────────

    #[test]
    fn delta_decode_i8_basic() {
        let mut data: Vec<i8> = vec![10, 5, -3, 2];
        delta_decode_i8(&mut data);
        assert_eq!(data, vec![10, 15, 12, 14]);
    }

    #[test]
    fn delta_decode_i16_basic() {
        let mut data: Vec<i16> = vec![100, -50, 25, -75];
        delta_decode_i16(&mut data);
        assert_eq!(data, vec![100, 50, 75, 0]);
    }

    #[test]
    fn delta_decode_wrapping() {
        // Wrapping arithmetic: 127i8 + 1i8 should wrap to -128.
        let mut data: Vec<i8> = vec![127, 1];
        delta_decode_i8(&mut data);
        assert_eq!(data[0], 127);
        assert_eq!(data[1], -128i8);
    }

    #[test]
    fn reject_invalid_magic() {
        let data = b"Not an XM file at all!!!!!!!!!!";
        assert!(parse(data).is_err());
    }

    #[test]
    fn reject_wrong_version() {
        let mut xm = make_minimal_xm_bytes();
        // Overwrite version field at offset 58 with 0x0102
        xm[58] = 0x02;
        xm[59] = 0x01;
        assert!(parse(&xm).is_err());
    }

    // ── Integration test: minimal empty module ────────────────────────────

    #[test]
    fn parse_minimal_module() {
        let data = make_minimal_xm_bytes();
        let module = parse(&data).expect("parse should succeed");

        assert_eq!(module.name, "Test");
        assert_eq!(module.tracker_name, "TestTracker");
        assert_eq!(module.version, 0x0104);
        assert_eq!(module.song_length, 1);
        assert_eq!(module.channel_count, 2);
        assert_eq!(module.default_tempo, 6);
        assert_eq!(module.default_bpm, 125);
        assert!(module.linear_frequencies);
        assert_eq!(module.pattern_order, vec![0]);
        assert_eq!(module.patterns.len(), 1);
        assert_eq!(module.instruments.len(), 0);

        let pat = &module.patterns[0];
        assert_eq!(pat.rows.len(), 32);
        assert_eq!(pat.rows[0].len(), 2);
        assert_eq!(pat.rows[0][0].note, XmNote::None);
    }

    #[test]
    fn parse_pattern_compressed_cells() {
        // Build a minimal XM with one pattern containing compressed note data.
        // One row, one channel: compressed note byte (flags=0x83 → note+inst
        // present), note=60, inst=1.
        // flags: 0x80 | 0x01 (note) | 0x02 (inst) = 0x83
        let packed: Vec<u8> = vec![0x83, 60, 1];
        let rows = unpack_pattern(&packed, 1, 1).expect("unpack_pattern");
        assert_eq!(rows[0][0].note, XmNote::On(60));
        assert_eq!(rows[0][0].instrument, 1);
        assert_eq!(rows[0][0].volume, 0);
        assert_eq!(rows[0][0].effect, 0);
        assert_eq!(rows[0][0].effect_param, 0);
    }

    #[test]
    fn parse_pattern_key_off() {
        // Uncompressed row: note=97 (key-off), rest zeros.
        let packed: Vec<u8> = vec![97, 0, 0, 0, 0];
        let rows = unpack_pattern(&packed, 1, 1).expect("unpack_pattern");
        assert_eq!(rows[0][0].note, XmNote::Off);
    }

    // ── Test helpers ─────────────────────────────────────────────────────

    /// Build the smallest valid XM binary: header + 1 empty 2-ch/32-row pattern.
    fn make_minimal_xm_bytes() -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();

        // ── Preamble (60 bytes) ──────────────────────────────────────────
        v.extend_from_slice(b"Extended Module: "); // 17
        let mut name = b"Test".to_vec();
        name.resize(20, 0);
        v.extend_from_slice(&name); // 20
        v.push(0x1A); // 1
        let mut tracker = b"TestTracker".to_vec();
        tracker.resize(20, 0);
        v.extend_from_slice(&tracker); // 20
        v.extend_from_slice(&0x0104u16.to_le_bytes()); // 2
        // Total: 60

        // ── Song header (276 bytes) ──────────────────────────────────────
        v.extend_from_slice(&276u32.to_le_bytes()); // header_size
        v.extend_from_slice(&1u16.to_le_bytes()); // song_length
        v.extend_from_slice(&0u16.to_le_bytes()); // restart_pos
        v.extend_from_slice(&2u16.to_le_bytes()); // channels
        v.extend_from_slice(&1u16.to_le_bytes()); // patterns
        v.extend_from_slice(&0u16.to_le_bytes()); // instruments
        v.extend_from_slice(&1u16.to_le_bytes()); // flags (linear freq)
        v.extend_from_slice(&6u16.to_le_bytes()); // tempo
        v.extend_from_slice(&125u16.to_le_bytes()); // BPM
        v.push(0x00); // order[0] = pattern 0
        v.extend(std::iter::repeat_n(0u8, 255)); // order[1..255]
        // header fields: 4+2+2+2+2+2+2+2+2 = 20, + 256 = 276

        // ── Pattern (9-byte header, 0 packed bytes) ──────────────────────
        v.extend_from_slice(&9u32.to_le_bytes()); // header_len
        v.push(0); // packing type
        v.extend_from_slice(&32u16.to_le_bytes()); // rows
        v.extend_from_slice(&0u16.to_le_bytes()); // packed_size = 0

        v
    }
}
