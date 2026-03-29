// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gravis UltraSound (GUS) `.pat` patch file loader.
//!
//! Produces an [`XmInstrument`] so the existing [`crate::player::Player`] can
//! play GUS patches without a separate engine.
//!
//! # File format overview
//! ```text
//! [file header — 239 bytes]
//!   magic     "GF1PATCH110\0"  (12)
//!   id        "ID#000002\0"    (10)
//!   desc      ASCII copyright  (60)
//!   instr     u8               (1)   number of instruments (always 1)
//!   voices    u8               (1)
//!   channels  u8               (1)
//!   wave_forms u16 LE          (2)   number of wave samples
//!   master_vol u16 LE          (2)
//!   data_size  u32 LE          (4)
//!   reserved   (146)
//!
//! [wave_forms × wave-sample block]
//!   wave header  96 bytes
//!   pcm data     wave_header.data_size bytes
//! ```
//!
//! # Pitch-correction formula
//! Each GUS sample has a `sample_rate` (recording rate) and `root_freq`
//! (the pitch it represents, in millihertz).  To play the sample at a target
//! frequency using the XM player's linear-frequency model we need:
//!
//! ```text
//! relative_note + finetune/128 = 12 × log₂(sample_rate / root_freq_hz)
//! ```
//!
//! # Verified against
//! Freepats project patches at `/usr/share/midi/freepats/`
//! (`000_Acoustic_Grand_Piano.pat`, `035_Kick_1.pat`, and others).

use anyhow::bail;

use crate::xm::{SampleLoopType, XmEnvelope, XmInstrument, XmSample};

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse a GUS `.pat` file from raw bytes, returning an [`XmInstrument`].
///
/// All wave samples in the patch become entries in
/// [`XmInstrument::samples`].  [`XmInstrument::note_to_sample`] is filled
/// in by matching each of the 96 playable XM notes to the sample whose
/// `[low_freq, high_freq]` range covers that note's frequency.
pub fn parse(data: &[u8]) -> anyhow::Result<XmInstrument> {
    if data.len() < 239 {
        bail!("not a GUS patch: file too short ({} bytes)", data.len());
    }
    if &data[0..12] != b"GF1PATCH110\0" {
        bail!(
            "not a GUS patch: bad magic {:?}",
            std::str::from_utf8(&data[0..12]).unwrap_or("?")
        );
    }

    let wave_forms = u16::from_le_bytes([data[85], data[86]]) as usize;
    if wave_forms == 0 {
        bail!("GUS patch has no wave samples");
    }

    // ── Parse wave samples ────────────────────────────────────────────────
    let mut offset = 239usize;
    let mut wave_hdrs: Vec<WaveHdr> = Vec::with_capacity(wave_forms);
    let mut xm_samples: Vec<XmSample> = Vec::with_capacity(wave_forms);

    for i in 0..wave_forms {
        if offset + 96 > data.len() {
            bail!("GUS patch truncated at wave header {i}");
        }
        let hdr = WaveHdr::read(&data[offset..offset + 96]);
        let pcm_end = offset + 96 + hdr.data_size as usize;
        if pcm_end > data.len() {
            bail!(
                "GUS patch truncated at wave PCM {i}: need {pcm_end}, have {}",
                data.len()
            );
        }
        let raw = &data[offset + 96..pcm_end];

        let pcm16 = convert_pcm(raw, &hdr);
        let (loop_start, loop_length, loop_type) = loop_params(&hdr);

        // Compute pitch correction:
        //   relative_note + finetune/128 = 12 * log2(sample_rate / root_freq_hz)
        let root_freq_hz = (hdr.root_freq as f64) / 1000.0;
        let correction = 12.0 * (hdr.sample_rate as f64 / root_freq_hz).log2();
        let relative_note = correction.round() as i32;
        let finetune_frac = (correction - correction.round()) * 128.0;

        // Clamp to i8 with a graceful fallback (sub-audible root frequencies
        // can push the correction slightly past 127 but are rare in practice).
        let relative_note_i8 = relative_note.clamp(-128, 127) as i8;
        let finetune_i8 = (finetune_frac.round() as i32).clamp(-128, 127) as i8;

        // GUS panning: 0–15 (0 = full left, 7/8 = centre, 15 = full right).
        let panning = ((hdr.panning as u16 * 255 + 7) / 15) as u8;

        let sample = XmSample {
            name: hdr.name.clone(),
            loop_start,
            loop_length,
            loop_type,
            volume: 64,
            finetune: finetune_i8,
            panning,
            relative_note: relative_note_i8,
            data: pcm16,
        };
        wave_hdrs.push(hdr);
        xm_samples.push(sample);
        offset = pcm_end;
    }

    // ── Build note-to-sample table ────────────────────────────────────────
    //
    // For each of the 96 playable XM notes, find the sample whose
    // [low_freq_hz, high_freq_hz] range covers the note's frequency.
    // If no single sample covers the frequency exactly, use the nearest one.
    let note_to_sample = build_note_map(&wave_hdrs);

    Ok(XmInstrument {
        name: wave_hdrs[0].name.clone(),
        note_to_sample,
        volume_envelope: XmEnvelope::default(),
        panning_envelope: XmEnvelope::default(),
        volume_fadeout: 0,
        vibrato_type: 0,
        vibrato_sweep: 0,
        vibrato_depth: 0,
        vibrato_rate: 0,
        samples: xm_samples,
    })
}

// ── Wave header ───────────────────────────────────────────────────────────────

/// Parsed GUS wave sample header (from 96 raw bytes at the current offset).
#[derive(Debug, Clone)]
struct WaveHdr {
    name: String,
    data_size: u32,
    loop_start: u32,
    loop_end: u32,
    sample_rate: u16,
    low_freq: u32,    // millihertz
    high_freq: u32,   // millihertz
    root_freq: u32,   // millihertz
    panning: u8,      // 0–15
    mode: u8,
    _scale_freq: i16,
    _scale_factor: u16,
}

impl WaveHdr {
    fn read(b: &[u8]) -> Self {
        let name_end = b[0..7].iter().position(|&x| x == 0).unwrap_or(7);
        let name = String::from_utf8_lossy(&b[0..name_end])
            .trim_end()
            .to_string();
        Self {
            name,
            data_size: u32::from_le_bytes(b[8..12].try_into().unwrap()),
            loop_start: u32::from_le_bytes(b[12..16].try_into().unwrap()),
            loop_end: u32::from_le_bytes(b[16..20].try_into().unwrap()),
            sample_rate: u16::from_le_bytes(b[20..22].try_into().unwrap()),
            low_freq: u32::from_le_bytes(b[22..26].try_into().unwrap()),
            high_freq: u32::from_le_bytes(b[26..30].try_into().unwrap()),
            root_freq: u32::from_le_bytes(b[30..34].try_into().unwrap()),
            panning: b[36],
            mode: b[55],
            _scale_freq: i16::from_le_bytes(b[56..58].try_into().unwrap()),
            _scale_factor: u16::from_le_bytes(b[58..60].try_into().unwrap()),
        }
    }

    fn is_16bit(&self) -> bool {
        self.mode & 0x01 != 0
    }
    fn is_unsigned(&self) -> bool {
        self.mode & 0x02 != 0
    }
    fn is_looping(&self) -> bool {
        self.mode & 0x04 != 0
    }
    fn is_pingpong(&self) -> bool {
        self.mode & 0x08 != 0
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert raw GUS PCM bytes to the Vec<i16> convention used by XmSample.
fn convert_pcm(raw: &[u8], hdr: &WaveHdr) -> Vec<i16> {
    if hdr.is_16bit() {
        // 16-bit LE samples — read as i16 directly, flipping sign if unsigned.
        raw.chunks_exact(2)
            .map(|c| {
                let v = i16::from_le_bytes([c[0], c[1]]);
                if hdr.is_unsigned() {
                    // unsigned 16-bit → signed: flip the MSB
                    v.wrapping_add(i16::MIN)
                } else {
                    v
                }
            })
            .collect()
    } else {
        // 8-bit samples — left-shift by 8 to fill the 16-bit range.
        raw.iter()
            .map(|&b| {
                let s: i8 = if hdr.is_unsigned() {
                    (b as i16 - 128) as i8
                } else {
                    b as i8
                };
                (s as i16) << 8
            })
            .collect()
    }
}

/// Derive XmSample loop parameters from the wave header.
///
/// GUS loop positions are stored in bytes; we convert to sample frames.
fn loop_params(hdr: &WaveHdr) -> (u32, u32, SampleLoopType) {
    if !hdr.is_looping() || hdr.loop_end <= hdr.loop_start {
        return (0, 0, SampleLoopType::None);
    }
    let bytes_per_sample = if hdr.is_16bit() { 2u32 } else { 1 };
    let start = hdr.loop_start / bytes_per_sample;
    let end = hdr.loop_end / bytes_per_sample;
    let length = end.saturating_sub(start);
    let loop_type = if hdr.is_pingpong() {
        SampleLoopType::PingPong
    } else {
        SampleLoopType::Forward
    };
    (start, length, loop_type)
}

/// Build the 96-entry note-to-sample index table.
///
/// For each XM note (0-indexed: 0 = C-0, 95 = B-7), compute the note's
/// output frequency and find the sample whose [low_freq, high_freq] range
/// (in millihertz) contains it.  Falls back to the nearest sample.
fn build_note_map(hdrs: &[WaveHdr]) -> [u8; 96] {
    let mut map = [0u8; 96];
    for (note_idx, slot) in map.iter_mut().enumerate() {
        // XM pitch for note index = note_idx (0-indexed semitone from C-0).
        // pitch_to_freq(n) = 8363 * 2^((n-60)/12)
        let pitch = note_idx as f64;
        let freq_mhz = (8363.0 * 2.0f64.powf((pitch - 60.0) / 12.0) * 1000.0) as u32;

        // Find the sample whose range contains this frequency.
        let mut best_idx = 0usize;
        let mut best_dist = u64::MAX;
        for (si, hdr) in hdrs.iter().enumerate() {
            if freq_mhz >= hdr.low_freq && freq_mhz <= hdr.high_freq {
                best_idx = si;
                break;
            }
            // Not in range — compute distance to the nearest boundary.
            let dist = if freq_mhz < hdr.low_freq {
                (hdr.low_freq - freq_mhz) as u64
            } else {
                (freq_mhz - hdr.high_freq) as u64
            };
            if dist < best_dist {
                best_dist = dist;
                best_idx = si;
            }
        }
        *slot = best_idx.min(255) as u8;
    }
    map
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const PIANO: &str = "/usr/share/midi/freepats/Tone_000/000_Acoustic_Grand_Piano.pat";
    const KICK: &str = "/usr/share/midi/freepats/Drum_000/035_Kick_1.pat";

    fn load(path: &str) -> Vec<u8> {
        std::fs::read(path).expect(path)
    }

    #[test]
    fn parse_piano_pat() {
        let data = load(PIANO);
        let instr = parse(&data).expect("parse piano .pat");
        assert_eq!(instr.samples.len(), 10, "piano should have 10 wave samples");
        // All samples must have non-empty PCM data.
        for (i, s) in instr.samples.iter().enumerate() {
            assert!(!s.data.is_empty(), "sample {i} has empty PCM data");
        }
    }

    #[test]
    fn parse_kick_pat() {
        let data = load(KICK);
        let instr = parse(&data).expect("parse kick .pat");
        assert_eq!(instr.samples.len(), 1);
        assert!(!instr.samples[0].data.is_empty());
    }

    #[test]
    fn note_map_coverage() {
        // Every note index must map to a valid sample.
        let data = load(PIANO);
        let instr = parse(&data).unwrap();
        let n = instr.samples.len() as u8;
        for (i, &s) in instr.note_to_sample.iter().enumerate() {
            assert!(s < n, "note {i} maps to sample {s}, but only {n} samples exist");
        }
    }

    #[test]
    fn piano_samples_have_loop() {
        let data = load(PIANO);
        let instr = parse(&data).unwrap();
        for (i, s) in instr.samples.iter().enumerate() {
            assert_ne!(
                s.loop_type,
                crate::xm::SampleLoopType::None,
                "piano sample {i} expected to loop"
            );
        }
    }

    #[test]
    fn relative_note_in_range() {
        let data = load(PIANO);
        let instr = parse(&data).unwrap();
        for (i, s) in instr.samples.iter().enumerate() {
            // relative_note is i8; check it's not the default zero for all samples
            // (the piano correction should be large and positive).
            assert!(
                s.relative_note > 50,
                "piano sample {i} relative_note={} expected > 50",
                s.relative_note
            );
        }
    }

    #[test]
    fn rejects_short_data() {
        assert!(parse(&[0u8; 10]).is_err());
    }

    #[test]
    fn rejects_wrong_magic() {
        let mut data = vec![0u8; 300];
        data[0..12].copy_from_slice(b"WRONGMAGIC\0X");
        assert!(parse(&data).is_err());
    }
}
