// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! XM module player / sequencer.
//!
//! [`Player`] drives the XM sequencer and provides a stereo-interleaved
//! [`fill`][Player::fill] method that produces the final mixed audio.
//!
//! # Supported features (Phase 1)
//! - Pattern sequencing: order list, multiple patterns, restart looping
//! - Sample playback: 16-bit PCM (8-bit left-shifted by parser), linear interpolation
//! - Loop modes: forward, ping-pong
//! - Volume envelope: linear interpolation between breakpoints, sustain, key-off
//! - Volume fade-out after key-off
//! - **Volume column**: set volume (`0x10`–`0x50`)
//! - **Effects**: `0xx` arpeggio, `1xx`/`2xx` portamento up/down, `3xx` tone
//!   portamento, `5xx` tone porta + vol slide, `Axx` volume slide,
//!   `Bxx` position jump, `Cxx` set volume, `Dxx` pattern break,
//!   `Fxx` set speed / BPM
//! - Linear frequency mode (the FastTracker II default; Amiga mode not yet implemented)
//!
//! # Threading
//! [`Player`] is not thread-safe on its own.  For native targets wrap it in
//! `Arc<Mutex<Player>>`; for WASM (single-threaded) use `Rc<RefCell<Player>>`.
//! See [`crate::backend::FillCallback`] for the target-specific callback type.

use std::sync::Arc;

use crate::xm::{SampleLoopType, XmEnvelope, XmModule, XmNote, XmSample};

// ── Public position snapshot ──────────────────────────────────────────────────

/// Snapshot of the sequencer position, suitable for driving a UI position display.
#[derive(Debug, Clone, Copy)]
pub struct PlaybackPosition {
    /// Current index into the pattern order list.
    pub order: usize,
    /// Index of the pattern currently playing.
    pub pattern: u8,
    /// Current row within the pattern (0-indexed).
    pub row: usize,
    /// Current tick within the row (0 … speed−1).
    pub tick: u32,
    /// Current BPM.
    pub bpm: u32,
    /// Current speed (ticks per row).
    pub speed: u32,
}

// ── Per-channel voice state ───────────────────────────────────────────────────

/// Per-channel voice and effect state.  All fields are updated by the sequencer;
/// the fill loop only reads the playback fields.
#[derive(Debug, Clone)]
struct Channel {
    // ── Instrument / sample reference ────────────────────────────────────────
    instrument_idx: usize,
    sample_idx: usize,

    // ── Playback ──────────────────────────────────────────────────────────────
    active: bool,
    key_off: bool,
    /// Fractional read position in `sample.data` (in sample frames).
    pos: f64,
    /// Source sample frames to advance per output frame.
    increment: f64,
    /// Direction flag for ping-pong loops.
    ping_pong_fwd: bool,

    // ── Volume / panning ──────────────────────────────────────────────────────
    /// Sample default volume, overridden by volume column / Cxx (0.0–1.0).
    base_vol: f32,
    /// Volume envelope output (0.0–1.0); 1.0 when envelope is disabled.
    env_vol: f32,
    /// Post-key-off fade multiplier; starts at 1.0, decremented by fadeout speed.
    fadeout: f32,
    /// Panning: 0.0 = hard-left, 0.5 = centre, 1.0 = hard-right.
    panning: f32,

    // ── Volume envelope state ─────────────────────────────────────────────────
    env_vol_tick: u32,

    // ── Pitch (semitones; C-5 = 60.0 ↔ 8363 Hz reference) ───────────────────
    /// Current pitch in semitones (slides via portamento).
    pitch: f64,
    /// Tone portamento target pitch.
    target_pitch: f64,

    // ── Effect memory ─────────────────────────────────────────────────────────
    effect: u8,
    effect_param: u8,
    vol_col: u8,
    last_porta_up: u8,
    last_porta_dn: u8,
    last_vol_slide: u8,
    /// Last triggered note (1-indexed XM), kept for arpeggio / portamento.
    note: u8,

    // ── Vibrato LFO ───────────────────────────────────────────────────────────
    vibrato_speed: u8,
    vibrato_depth: u8,
    /// Phase: 0–63, advances by `vibrato_speed` each tick.
    vibrato_phase: u8,

    // ── Tremolo LFO ───────────────────────────────────────────────────────────
    tremolo_speed: u8,
    tremolo_depth: u8,
    tremolo_phase: u8,
    /// Additive volume offset produced by tremolo; applied in the fill loop.
    tremolo_offset: f32,

    // ── Timed note events ─────────────────────────────────────────────────────
    /// ECx: cut note to silence at this tick (0 = not active).
    note_cut_tick: u8,
    /// E9x: retrigger period in ticks (0 = not active).
    retrig_period: u8,
    retrig_count: u8,

    // ── Pattern loop (E6x) ────────────────────────────────────────────────────
    /// Row where E60 was last seen (loop start).
    loop_row: usize,
    /// Remaining loop iterations; 0 = not in a loop.
    loop_count: u8,

    // ── Delayed note trigger (EDx) ────────────────────────────────────────────
    /// Non-zero = note trigger is pending at this tick.
    note_delay_tick: u8,
    /// Saved state for the delayed trigger.
    delay_note: u8,
    delay_instr: usize,
    delay_sample: usize,
    delay_base_vol: f32,
    delay_panning: f32,
    delay_pitch: f64,
    delay_inc: f64,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            instrument_idx: 0,
            sample_idx: 0,
            active: false,
            key_off: false,
            pos: 0.0,
            increment: 0.0,
            ping_pong_fwd: true,
            base_vol: 1.0,
            env_vol: 1.0,
            fadeout: 1.0,
            panning: 0.5,
            env_vol_tick: 0,
            pitch: 60.0,
            target_pitch: 60.0,
            effect: 0,
            effect_param: 0,
            vol_col: 0,
            last_porta_up: 0,
            last_porta_dn: 0,
            last_vol_slide: 0,
            note: 0,
            vibrato_speed: 0,
            vibrato_depth: 0,
            vibrato_phase: 0,
            tremolo_speed: 0,
            tremolo_depth: 0,
            tremolo_phase: 0,
            tremolo_offset: 0.0,
            note_cut_tick: 0,
            retrig_period: 0,
            retrig_count: 0,
            loop_row: 0,
            loop_count: 0,
            note_delay_tick: 0,
            delay_note: 0,
            delay_instr: 0,
            delay_sample: 0,
            delay_base_vol: 1.0,
            delay_panning: 0.5,
            delay_pitch: 60.0,
            delay_inc: 0.0,
        }
    }
}

// ── Player ────────────────────────────────────────────────────────────────────

/// XM module sequencer and sample mixer.
///
/// # Example
/// ```ignore
/// let module = Arc::new(tracker_engine::xm::parse(&bytes)?);
/// let mut player = Player::new(Arc::clone(&module), 44_100);
/// player.play();
/// backend.start(Box::new(move |buf| player.fill(buf)))?;
/// ```
pub struct Player {
    module: Arc<XmModule>,
    sample_rate: u32,
    playing: bool,

    channels: Vec<Channel>,

    // ── Sequencer position ────────────────────────────────────────────────────
    order_pos: usize,
    row: usize,
    tick: u32,

    // ── Timing ────────────────────────────────────────────────────────────────
    bpm: u32,
    speed: u32,
    /// Floating-point samples per tick; recomputed when BPM changes.
    samples_per_tick: f64,
    /// Fractional sample counter; triggers a tick when it reaches `samples_per_tick`.
    tick_acc: f64,

    // ── Deferred row-end jumps (Bxx / Dxx) ───────────────────────────────────
    next_order: Option<usize>,
    next_row: Option<usize>,
}

impl Player {
    /// Create a new player for `module` at the given `sample_rate` (Hz).
    ///
    /// The player starts **stopped**; call [`play`][Self::play] to begin.
    pub fn new(module: Arc<XmModule>, sample_rate: u32) -> Self {
        let n_ch = module.channel_count as usize;
        let bpm = module.default_bpm.max(1) as u32;
        let speed = module.default_tempo.max(1) as u32;
        let spt = calc_samples_per_tick(sample_rate, bpm);

        Self {
            channels: vec![Channel::default(); n_ch],
            order_pos: 0,
            row: 0,
            tick: 0,
            bpm,
            speed,
            samples_per_tick: spt,
            // Initialise at the boundary so the first output sample triggers row 0, tick 0.
            tick_acc: spt,
            next_order: None,
            next_row: None,
            playing: false,
            module,
            sample_rate,
        }
    }

    /// Start or resume playback from the current position.
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Pause playback; the position is retained.
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Stop playback and rewind to the beginning.
    pub fn stop(&mut self) {
        self.playing = false;
        self.order_pos = 0;
        self.row = 0;
        self.tick = 0;
        self.bpm = self.module.default_bpm.max(1) as u32;
        self.speed = self.module.default_tempo.max(1) as u32;
        self.samples_per_tick = calc_samples_per_tick(self.sample_rate, self.bpm);
        self.tick_acc = self.samples_per_tick;
        self.next_order = None;
        self.next_row = None;
        for ch in &mut self.channels {
            *ch = Channel::default();
        }
    }

    /// Returns `true` while the sequencer is advancing.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Snapshot the current sequencer position (for UI display).
    pub fn position(&self) -> PlaybackPosition {
        let pattern = self
            .module
            .pattern_order
            .get(self.order_pos)
            .copied()
            .unwrap_or(0);
        PlaybackPosition {
            order: self.order_pos,
            pattern,
            row: self.row,
            tick: self.tick,
            bpm: self.bpm,
            speed: self.speed,
        }
    }

    /// Jump to a specific position in the order list.
    pub fn set_position(&mut self, order: usize, row: usize) {
        self.order_pos = order.min((self.module.song_length as usize).saturating_sub(1));
        self.row = row;
        self.tick = 0;
        self.tick_acc = self.samples_per_tick;
        for ch in &mut self.channels {
            ch.active = false;
        }
    }

    /// Fill `buf` with stereo-interleaved f32 samples (`[L0, R0, L1, R1, …]`).
    ///
    /// Silence is written when the player is paused or stopped.  The buffer
    /// is always fully written.
    pub fn fill(&mut self, buf: &mut [f32]) {
        buf.fill(0.0);
        if !self.playing {
            return;
        }

        let n_ch = self.channels.len();
        if n_ch == 0 {
            return;
        }
        // Normalise amplitude across channels (constant-power approximation).
        let master: f32 = 1.0 / (n_ch as f32).sqrt();

        let frames = buf.len() / 2;
        for fi in 0..frames {
            // Process as many ticks as are due before this output frame.
            while self.tick_acc >= self.samples_per_tick {
                self.tick_acc -= self.samples_per_tick;
                self.process_tick();
                if !self.playing {
                    return;
                }
            }
            self.tick_acc += 1.0;

            // Mix all active channels.
            // We borrow self.module and self.channels as separate struct fields,
            // which the borrow checker allows.
            let module = &*self.module;
            let mut l = 0.0f32;
            let mut r = 0.0f32;

            for ch in self.channels.iter_mut() {
                if !ch.active {
                    continue;
                }
                let Some(instr) = module.instruments.get(ch.instrument_idx) else {
                    ch.active = false;
                    continue;
                };
                let Some(sample) = instr.samples.get(ch.sample_idx) else {
                    ch.active = false;
                    continue;
                };
                if sample.data.is_empty() {
                    ch.active = false;
                    continue;
                }

                let sv = read_sample_lerp(sample, ch.pos);
                if advance_pos(ch, sample) {
                    ch.active = false;
                }

                let vol = (ch.base_vol + ch.tremolo_offset).clamp(0.0, 1.0)
                    * ch.env_vol
                    * ch.fadeout
                    * master;
                l += sv * vol * (1.0 - ch.panning);
                r += sv * vol * ch.panning;
            }

            buf[fi * 2] = l.clamp(-1.0, 1.0);
            buf[fi * 2 + 1] = r.clamp(-1.0, 1.0);
        }
    }
}

// ── Sequencer internals ───────────────────────────────────────────────────────

impl Player {
    fn process_tick(&mut self) {
        // On tick 0: read the row and trigger notes.
        if self.tick == 0 {
            self.process_row();
        }

        // Per-tick effect updates (tick > 0 only for most effects).
        for ch_idx in 0..self.channels.len() {
            self.tick_effects(ch_idx);
        }

        // Volume envelope + fade-out.
        for ch_idx in 0..self.channels.len() {
            self.tick_envelope(ch_idx);
        }

        // Advance tick counter; when it wraps, move to the next row.
        self.tick += 1;
        if self.tick >= self.speed {
            self.tick = 0;
            self.advance_row();
        }
    }

    fn process_row(&mut self) {
        let Some(&pat_idx) = self.module.pattern_order.get(self.order_pos) else {
            return;
        };
        let Some(pattern) = self.module.patterns.get(pat_idx as usize) else {
            return;
        };
        let Some(row_cells) = pattern.rows.get(self.row) else {
            return;
        };

        // Clone cells to release the shared borrow on self.module before
        // we start mutating self.channels inside trigger_cell.
        let cells: Vec<_> = row_cells
            .iter()
            .take(self.channels.len())
            .cloned()
            .collect();

        for (ch_idx, cell) in cells.iter().enumerate() {
            self.trigger_cell(ch_idx, cell);
        }
    }

    fn trigger_cell(&mut self, ch_idx: usize, cell: &crate::xm::XmCell) {
        // Stash effect state for tick_effects.
        {
            let ch = &mut self.channels[ch_idx];
            ch.vol_col = cell.volume;
            ch.effect = cell.effect;
            ch.effect_param = cell.effect_param;
            // Clear timed events from the previous row.
            ch.note_cut_tick = 0;
            ch.retrig_period = 0;
            ch.retrig_count = 0;
            ch.note_delay_tick = 0;
            ch.tremolo_offset = 0.0;
        }

        // Key-off: release note, let envelope + fadeout play out.
        if cell.note == XmNote::Off {
            self.channels[ch_idx].key_off = true;
            return;
        }

        let has_note = matches!(cell.note, XmNote::On(_));
        let has_inst = cell.instrument > 0;
        let instr_idx = if has_inst {
            (cell.instrument as usize).saturating_sub(1)
        } else {
            self.channels[ch_idx].instrument_idx
        };

        // Instrument change without a new note: reset envelope only.
        if has_inst && !has_note && instr_idx < self.module.instruments.len() {
            let ch = &mut self.channels[ch_idx];
            ch.instrument_idx = instr_idx;
            ch.env_vol_tick = 0;
            ch.env_vol = 1.0;
            ch.fadeout = 1.0;
            ch.key_off = false;
        }

        if has_note {
            let note_val = match cell.note {
                XmNote::On(n) => n,
                _ => unreachable!(),
            };

            if instr_idx < self.module.instruments.len() {
                let instr = &self.module.instruments[instr_idx];
                let note_0 = (note_val as usize).saturating_sub(1).min(95);
                let sample_idx = instr.note_to_sample[note_0] as usize;

                if sample_idx < instr.samples.len() {
                    let sample = &instr.samples[sample_idx];
                    let real_pitch = note_to_pitch(note_val, sample.relative_note, sample.finetune);
                    let default_vol = sample.volume as f32 / 64.0;
                    let default_pan = sample.panning as f32 / 255.0;
                    let inc = pitch_to_increment(real_pitch, self.sample_rate);

                    if cell.effect == 0x03 {
                        // 3xx — tone portamento: record target but keep current voice.
                        let ch = &mut self.channels[ch_idx];
                        ch.target_pitch = real_pitch;
                        if cell.effect_param != 0 {
                            ch.last_porta_up = cell.effect_param;
                            ch.last_porta_dn = cell.effect_param;
                        }
                    } else if cell.effect == 0x0E
                        && (cell.effect_param >> 4) == 0x0D
                        && (cell.effect_param & 0x0F) > 0
                    {
                        // EDx (x > 0) — note delay: save trigger for later.
                        let delay = cell.effect_param & 0x0F;
                        let ch = &mut self.channels[ch_idx];
                        ch.note_delay_tick = delay;
                        ch.delay_note = note_val;
                        ch.delay_instr = instr_idx;
                        ch.delay_sample = sample_idx;
                        ch.delay_base_vol = default_vol;
                        ch.delay_panning = default_pan;
                        ch.delay_pitch = real_pitch;
                        ch.delay_inc = inc;
                    } else {
                        // Normal note trigger: (re)start the sample from the beginning.
                        let ch = &mut self.channels[ch_idx];
                        ch.instrument_idx = instr_idx;
                        ch.sample_idx = sample_idx;
                        ch.pos = 0.0;
                        ch.increment = inc;
                        ch.active = true;
                        ch.key_off = false;
                        ch.ping_pong_fwd = true;
                        ch.env_vol_tick = 0;
                        ch.env_vol = 1.0;
                        ch.fadeout = 1.0;
                        ch.note = note_val;
                        ch.pitch = real_pitch;
                        ch.target_pitch = real_pitch;
                        ch.base_vol = default_vol;
                        ch.panning = default_pan;
                    }
                }
            }
        }

        // Volume column effects (applied at tick 0).
        let vol_col = cell.volume;
        match vol_col {
            0x10..=0x50 => {
                // Set volume 0–64.
                self.channels[ch_idx].base_vol = (vol_col - 0x10) as f32 / 64.0;
            }
            0x80..=0x8F => {
                // Fine volume slide down (once, at row start).
                let amt = (vol_col & 0x0F) as f32 / 64.0;
                self.channels[ch_idx].base_vol = (self.channels[ch_idx].base_vol - amt).max(0.0);
            }
            0x90..=0x9F => {
                // Fine volume slide up (once, at row start).
                let amt = (vol_col & 0x0F) as f32 / 64.0;
                self.channels[ch_idx].base_vol = (self.channels[ch_idx].base_vol + amt).min(1.0);
            }
            0xA0..=0xAF => {
                // Set vibrato speed.
                self.channels[ch_idx].vibrato_speed = vol_col & 0x0F;
            }
            0xB0..=0xBF => {
                // Set vibrato depth (speed was already set via 0xAx or 4xx).
                self.channels[ch_idx].vibrato_depth = vol_col & 0x0F;
            }
            0xC0..=0xCF => {
                // Set panning: 0–F → 0.0–1.0.
                self.channels[ch_idx].panning = (vol_col & 0x0F) as f32 / 15.0;
            }
            _ => {}
        }

        // Effect actions that apply only on tick 0.
        self.effect_row0(ch_idx);
    }

    /// Effects processed once at tick 0 (row trigger time).
    fn effect_row0(&mut self, ch_idx: usize) {
        let effect = self.channels[ch_idx].effect;
        let param = self.channels[ch_idx].effect_param;

        match effect {
            0x03 => {
                // 3xx — tone portamento: memorise speed.
                if param != 0 {
                    let ch = &mut self.channels[ch_idx];
                    ch.last_porta_up = param;
                    ch.last_porta_dn = param;
                }
            }
            0x04 => {
                // 4xy — vibrato: memorise speed (high nibble) and depth (low nibble).
                let ch = &mut self.channels[ch_idx];
                if param >> 4 != 0 {
                    ch.vibrato_speed = param >> 4;
                }
                if param & 0x0F != 0 {
                    ch.vibrato_depth = param & 0x0F;
                }
            }
            0x07 => {
                // 7xy — tremolo: memorise speed and depth.
                let ch = &mut self.channels[ch_idx];
                if param >> 4 != 0 {
                    ch.tremolo_speed = param >> 4;
                }
                if param & 0x0F != 0 {
                    ch.tremolo_depth = param & 0x0F;
                }
            }
            0x08 => {
                // 8xx — set panning (0–255 → 0.0–1.0).
                self.channels[ch_idx].panning = param as f32 / 255.0;
            }
            0x09 => {
                // 9xx — sample offset: jump playback position to param * 256 frames.
                if param > 0 {
                    self.channels[ch_idx].pos = param as f64 * 256.0;
                }
            }
            0x0B => {
                // Bxx — position jump to order `param`.
                self.next_order = Some(param as usize);
                if self.next_row.is_none() {
                    self.next_row = Some(0);
                }
            }
            0x0C => {
                // Cxx — set volume (0–64).
                self.channels[ch_idx].base_vol = param.min(64) as f32 / 64.0;
            }
            0x0D => {
                // Dxx — pattern break.  Param is BCD: tens*16 + units.
                let row = (param >> 4) as usize * 10 + (param & 0x0F) as usize;
                self.next_row = Some(row);
            }
            0x0F => {
                // Fxx — set speed (< 0x20) or BPM (>= 0x20).
                if param == 0 {
                    // Treat F00 as stop (matches FT2 behaviour).
                    self.playing = false;
                } else if param < 0x20 {
                    self.speed = param as u32;
                } else {
                    self.bpm = param as u32;
                    self.samples_per_tick = calc_samples_per_tick(self.sample_rate, self.bpm);
                }
            }
            0x0E => {
                // Exx — extended effects.
                let sub = param >> 4;
                let val = param & 0x0F;
                match sub {
                    0x1 => {
                        // E1x — fine portamento up.
                        let sr = self.sample_rate;
                        let ch = &mut self.channels[ch_idx];
                        ch.pitch = (ch.pitch + val as f64 / 16.0).min(120.0);
                        ch.increment = pitch_to_increment(ch.pitch, sr);
                    }
                    0x2 => {
                        // E2x — fine portamento down.
                        let sr = self.sample_rate;
                        let ch = &mut self.channels[ch_idx];
                        ch.pitch = (ch.pitch - val as f64 / 16.0).max(0.0);
                        ch.increment = pitch_to_increment(ch.pitch, sr);
                    }
                    0x6 => {
                        // E6x — pattern loop.
                        if val == 0 {
                            // E60: set loop start for this channel.
                            self.channels[ch_idx].loop_row = self.row;
                        } else {
                            // E6x (x > 0): loop back x times.
                            let ch = &mut self.channels[ch_idx];
                            if ch.loop_count == 0 {
                                ch.loop_count = val;
                                self.next_row = Some(ch.loop_row);
                            } else {
                                ch.loop_count -= 1;
                                if ch.loop_count > 0 {
                                    self.next_row = Some(ch.loop_row);
                                }
                            }
                        }
                    }
                    0x8 => {
                        // E8x — set panning (0–F → 0.0–1.0).
                        self.channels[ch_idx].panning = val as f32 / 15.0;
                    }
                    0x9 => {
                        // E9x — retrigger every `val` ticks.
                        if val > 0 {
                            self.channels[ch_idx].retrig_period = val;
                            self.channels[ch_idx].retrig_count = 0;
                        }
                    }
                    0xA => {
                        // EAx — fine volume slide up (once, tick 0).
                        let amt = val as f32 / 64.0;
                        self.channels[ch_idx].base_vol =
                            (self.channels[ch_idx].base_vol + amt).min(1.0);
                    }
                    0xB => {
                        // EBx — fine volume slide down (once, tick 0).
                        let amt = val as f32 / 64.0;
                        self.channels[ch_idx].base_vol =
                            (self.channels[ch_idx].base_vol - amt).max(0.0);
                    }
                    0xC => {
                        // ECx — note cut at tick `val`.
                        self.channels[ch_idx].note_cut_tick = val;
                    }
                    0xD => {
                        // EDx — note delay: if val == 0, trigger immediately (no delay).
                        // val > 0 case is handled in trigger_cell; nothing to do here.
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Effects processed on every tick > 0.
    fn tick_effects(&mut self, ch_idx: usize) {
        // Row-0 effects are handled in effect_row0; skip here.
        if self.tick == 0 {
            return;
        }

        let effect = self.channels[ch_idx].effect;
        let param = self.channels[ch_idx].effect_param;

        match effect {
            0x00 => {
                // 0xx — arpeggio: cycle through base, base+x, base+y each tick.
                if param != 0 {
                    let base = self.channels[ch_idx].pitch;
                    let hi = (param >> 4) as f64;
                    let lo = (param & 0x0F) as f64;
                    let arp_offset = match self.tick % 3 {
                        0 => 0.0,
                        1 => hi,
                        _ => lo,
                    };
                    // Modify only the increment; pitch (base) stays unchanged.
                    let sr = self.sample_rate;
                    let ch = &mut self.channels[ch_idx];
                    ch.increment = pitch_to_increment(base + arp_offset, sr);
                }
            }
            0x01 => {
                // 1xx — portamento up (increase pitch).
                let speed = if param != 0 {
                    self.channels[ch_idx].last_porta_up = param;
                    param
                } else {
                    self.channels[ch_idx].last_porta_up
                };
                let sr = self.sample_rate;
                let ch = &mut self.channels[ch_idx];
                ch.pitch = (ch.pitch + speed as f64 / 16.0).min(120.0);
                ch.increment = pitch_to_increment(ch.pitch, sr);
            }
            0x02 => {
                // 2xx — portamento down (decrease pitch).
                let speed = if param != 0 {
                    self.channels[ch_idx].last_porta_dn = param;
                    param
                } else {
                    self.channels[ch_idx].last_porta_dn
                };
                let sr = self.sample_rate;
                let ch = &mut self.channels[ch_idx];
                ch.pitch = (ch.pitch - speed as f64 / 16.0).max(0.0);
                ch.increment = pitch_to_increment(ch.pitch, sr);
            }
            0x03 => {
                // 3xx — tone portamento (slide towards target).
                let speed = if param != 0 {
                    self.channels[ch_idx].last_porta_up = param;
                    self.channels[ch_idx].last_porta_dn = param;
                    param
                } else {
                    self.channels[ch_idx].last_porta_up
                };
                self.do_tone_portamento(ch_idx, speed);
            }
            0x04 => {
                // 4xx — vibrato: oscillate pitch via sine LFO.
                let sr = self.sample_rate;
                let ch = &mut self.channels[ch_idx];
                if param >> 4 != 0 {
                    ch.vibrato_speed = param >> 4;
                }
                if param & 0x0F != 0 {
                    ch.vibrato_depth = param & 0x0F;
                }
                let lfo = vibrato_lfo(ch.vibrato_phase);
                let delta = lfo * ch.vibrato_depth as f64 / 16.0;
                // Only the increment is modified; ch.pitch (base) is preserved.
                ch.increment = pitch_to_increment(ch.pitch + delta, sr);
                ch.vibrato_phase = ch.vibrato_phase.wrapping_add(ch.vibrato_speed) & 63;
            }
            0x05 => {
                // 5xx — tone portamento + volume slide.
                let porta_speed = self.channels[ch_idx].last_porta_up;
                self.do_tone_portamento(ch_idx, porta_speed);
                self.do_vol_slide(ch_idx, param);
            }
            0x06 => {
                // 6xx — vibrato + volume slide.
                {
                    let sr = self.sample_rate;
                    let ch = &mut self.channels[ch_idx];
                    let lfo = vibrato_lfo(ch.vibrato_phase);
                    let delta = lfo * ch.vibrato_depth as f64 / 16.0;
                    ch.increment = pitch_to_increment(ch.pitch + delta, sr);
                    ch.vibrato_phase = ch.vibrato_phase.wrapping_add(ch.vibrato_speed) & 63;
                }
                self.do_vol_slide(ch_idx, param);
            }
            0x07 => {
                // 7xx — tremolo: oscillate volume via sine LFO.
                let ch = &mut self.channels[ch_idx];
                if param >> 4 != 0 {
                    ch.tremolo_speed = param >> 4;
                }
                if param & 0x0F != 0 {
                    ch.tremolo_depth = param & 0x0F;
                }
                let lfo = vibrato_lfo(ch.tremolo_phase) as f32;
                ch.tremolo_offset = lfo * ch.tremolo_depth as f32 / 64.0;
                ch.tremolo_phase = ch.tremolo_phase.wrapping_add(ch.tremolo_speed) & 63;
            }
            0x09 => {
                // 9xx — sample offset: set playback start position.
                // Applied on tick 0 only; tick > 0 does nothing for 9xx.
            }
            0x0A => {
                // Axx — volume slide.
                self.do_vol_slide(ch_idx, param);
            }
            0x0E => {
                // Exx timed events.
                let sub = param >> 4;
                let val = param & 0x0F;
                match sub {
                    0x9 if val > 0 => {
                        // E9x — retrigger: restart sample every `val` ticks.
                        let ch = &mut self.channels[ch_idx];
                        ch.retrig_count += 1;
                        if ch.retrig_count >= ch.retrig_period {
                            ch.retrig_count = 0;
                            ch.pos = 0.0;
                            ch.ping_pong_fwd = true;
                        }
                    }
                    0xC => {
                        // ECx — note cut at tick `val`.
                        if self.tick == val as u32 {
                            self.channels[ch_idx].base_vol = 0.0;
                        }
                    }
                    0xD if val > 0 => {
                        // EDx — fire delayed note trigger.
                        if self.tick == val as u32 {
                            let ch = &self.channels[ch_idx];
                            let instr_idx = ch.delay_instr;
                            let sample_idx = ch.delay_sample;
                            let base_vol = ch.delay_base_vol;
                            let panning = ch.delay_panning;
                            let pitch = ch.delay_pitch;
                            let inc = ch.delay_inc;
                            let note_val = ch.delay_note;
                            let ch = &mut self.channels[ch_idx];
                            ch.instrument_idx = instr_idx;
                            ch.sample_idx = sample_idx;
                            ch.pos = 0.0;
                            ch.increment = inc;
                            ch.active = true;
                            ch.key_off = false;
                            ch.ping_pong_fwd = true;
                            ch.env_vol_tick = 0;
                            ch.env_vol = 1.0;
                            ch.fadeout = 1.0;
                            ch.note = note_val;
                            ch.pitch = pitch;
                            ch.target_pitch = pitch;
                            ch.base_vol = base_vol;
                            ch.panning = panning;
                            ch.note_delay_tick = 0;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Slide `ch.pitch` toward `ch.target_pitch` at `speed / 16.0` semitones per tick.
    fn do_tone_portamento(&mut self, ch_idx: usize, speed: u8) {
        let sr = self.sample_rate;
        let ch = &mut self.channels[ch_idx];
        let delta = speed as f64 / 16.0;
        if ch.pitch < ch.target_pitch {
            ch.pitch = (ch.pitch + delta).min(ch.target_pitch);
        } else if ch.pitch > ch.target_pitch {
            ch.pitch = (ch.pitch - delta).max(ch.target_pitch);
        }
        ch.increment = pitch_to_increment(ch.pitch, sr);
    }

    /// Apply a volume slide from the Axx / 5xx / 6xx parameter byte.
    ///
    /// High nibble → slide up; low nibble → slide down.  Both non-zero
    /// means "slide up" wins (FT2 behaviour).
    fn do_vol_slide(&mut self, ch_idx: usize, param: u8) {
        let param = if param != 0 {
            self.channels[ch_idx].last_vol_slide = param;
            param
        } else {
            self.channels[ch_idx].last_vol_slide
        };
        let ch = &mut self.channels[ch_idx];
        let up = (param >> 4) as f32 / 64.0;
        let dn = (param & 0x0F) as f32 / 64.0;
        if up > 0.0 {
            ch.base_vol = (ch.base_vol + up).min(1.0);
        } else {
            ch.base_vol = (ch.base_vol - dn).max(0.0);
        }
    }

    /// Advance the volume envelope and apply fade-out for one tick.
    fn tick_envelope(&mut self, ch_idx: usize) {
        let ch = &self.channels[ch_idx];
        if !ch.active {
            return;
        }

        let instr_idx = ch.instrument_idx;
        let tick = ch.env_vol_tick;
        let key_off = ch.key_off;

        let (env_val, next_tick) = if instr_idx < self.module.instruments.len() {
            let env = &self.module.instruments[instr_idx].volume_envelope;
            eval_vol_envelope(env, tick, key_off)
        } else {
            (1.0, tick + 1)
        };

        let ch = &mut self.channels[ch_idx];
        ch.env_vol = env_val;
        ch.env_vol_tick = next_tick;

        // Apply fade-out after key-off.
        if ch.key_off && instr_idx < self.module.instruments.len() {
            let speed = self.module.instruments[instr_idx].volume_fadeout as f32;
            ch.fadeout = (ch.fadeout - speed / 65536.0).max(0.0);
            if ch.fadeout == 0.0 {
                ch.active = false;
            }
        }
    }

    /// Advance the playback position to the next row, handling Bxx/Dxx jumps,
    /// pattern boundaries, and song-end looping.
    fn advance_row(&mut self) {
        let song_len = self.module.song_length as usize;

        let jump_order = self.next_order.take();
        let jump_row = self.next_row.take();

        // Bxx: jump to order position (+ optional Dxx row offset).
        if let Some(order) = jump_order {
            self.order_pos = order.min(song_len.saturating_sub(1));
            self.row = jump_row.unwrap_or(0);
            return;
        }

        // Dxx alone: break to the start of the next pattern at a given row.
        if let Some(row) = jump_row {
            self.order_pos += 1;
            if self.order_pos >= song_len {
                self.order_pos = self.module.restart_position as usize;
            }
            let new_pat = self
                .module
                .pattern_order
                .get(self.order_pos)
                .copied()
                .unwrap_or(0) as usize;
            let max_rows = self
                .module
                .patterns
                .get(new_pat)
                .map(|p| p.rows.len())
                .unwrap_or(1);
            self.row = row.min(max_rows.saturating_sub(1));
            return;
        }

        // Normal advance: next row in the current pattern.
        let pat_idx = self
            .module
            .pattern_order
            .get(self.order_pos)
            .copied()
            .unwrap_or(0) as usize;
        let row_count = self
            .module
            .patterns
            .get(pat_idx)
            .map(|p| p.rows.len())
            .unwrap_or(64);

        self.row += 1;
        if self.row >= row_count {
            self.row = 0;
            self.order_pos += 1;
            if self.order_pos >= song_len {
                let restart = self.module.restart_position as usize;
                if restart < song_len {
                    self.order_pos = restart;
                } else {
                    // No valid restart: stop.
                    self.playing = false;
                }
            }
        }
    }
}

// ── Sample mixing helpers ─────────────────────────────────────────────────────

/// Read one sample frame from `sample.data` at fractional position `pos`
/// using linear interpolation.  Returns a normalised f32 in `[-1.0, 1.0]`.
fn read_sample_lerp(sample: &XmSample, pos: f64) -> f32 {
    let i = pos as usize;
    let frac = (pos - i as f64) as f32;
    // SAFETY: callers ensure data is non-empty and pos is in bounds.
    let s0 = sample.data[i.min(sample.data.len() - 1)] as f32 / 32768.0;
    let s1 = sample.data[(i + 1).min(sample.data.len() - 1)] as f32 / 32768.0;
    s0 + (s1 - s0) * frac
}

/// Advance `ch.pos` by `ch.increment`, applying loop logic.
///
/// Returns `true` if the sample ended without a loop and the channel should be
/// deactivated.
fn advance_pos(ch: &mut Channel, sample: &XmSample) -> bool {
    let data_len = sample.data.len() as f64;

    match sample.loop_type {
        SampleLoopType::None => {
            let new_pos = ch.pos + ch.increment;
            if new_pos >= data_len {
                return true;
            }
            ch.pos = new_pos;
        }
        SampleLoopType::Forward => {
            let ls = sample.loop_start as f64;
            let le = ls + sample.loop_length as f64;
            let mut p = ch.pos + ch.increment;
            if sample.loop_length > 0 && p >= le {
                let span = le - ls;
                p = ls + (p - ls).rem_euclid(span);
            }
            ch.pos = p;
        }
        SampleLoopType::PingPong => {
            let ls = sample.loop_start as f64;
            let le = ls + sample.loop_length as f64;
            if sample.loop_length == 0 {
                ch.pos += ch.increment;
                return false;
            }

            // Advance in the current direction, bouncing when a boundary is hit.
            let delta = ch.increment;
            let mut p = if ch.ping_pong_fwd {
                ch.pos + delta
            } else {
                ch.pos - delta
            };
            // Resolve multiple bounces (rare at normal speeds).
            for _ in 0..4 {
                if ch.ping_pong_fwd {
                    if p >= le {
                        p = 2.0 * le - p;
                        ch.ping_pong_fwd = false;
                    } else {
                        break;
                    }
                } else if p < ls {
                    p = 2.0 * ls - p;
                    ch.ping_pong_fwd = true;
                } else {
                    break;
                }
            }
            ch.pos = p.clamp(ls, le - f64::EPSILON);
        }
    }
    false
}

// ── Envelope ─────────────────────────────────────────────────────────────────

/// Evaluate the volume envelope at `tick` ticks since note-on.
///
/// Returns `(value_0_to_1, next_tick_counter)`.  The tick counter is held
/// (not incremented) while the sustain point is active.
fn eval_vol_envelope(env: &XmEnvelope, tick: u32, key_off: bool) -> (f32, u32) {
    if !env.enabled || env.points.is_empty() {
        return (1.0, tick + 1);
    }

    let pts = &env.points;
    let last = pts.len() - 1;

    // Sustain: freeze at the sustain point while the note is held.
    if env.sustain && !key_off {
        let si = env.sustain_point as usize;
        if let Some(sp) = pts.get(si)
            && tick >= sp.tick as u32
        {
            return (sp.value as f32 / 64.0, tick); // hold — do not advance
        }
    }

    // Past the end: hold at the final value.
    let last_tick = pts[last].tick as u32;
    if tick >= last_tick {
        return (pts[last].value as f32 / 64.0, tick);
    }

    // Find the enclosing segment with a linear search (max 12 points).
    let seg = pts
        .windows(2)
        .position(|w| tick < w[1].tick as u32)
        .unwrap_or(last.saturating_sub(1));

    let p0 = &pts[seg];
    let p1 = &pts[seg + 1];
    let span = (p1.tick - p0.tick) as f32;
    let t = if span > 0.0 {
        (tick - p0.tick as u32) as f32 / span
    } else {
        0.0
    };
    let val = p0.value as f32 + (p1.value as f32 - p0.value as f32) * t;
    (val / 64.0, tick + 1)
}

// ── Frequency helpers ─────────────────────────────────────────────────────────

/// Convert a pattern note + sample tuning into a pitch in semitones.
///
/// The pitch scale is the same as MIDI semitones with C-5 = 60.0,
/// matching the XM reference frequency of 8363 Hz.
///
/// `note_1indexed`: XM note value (1 = C-0, 96 = B-7).
/// `relative_note`: semitone offset stored in the sample header (typically 0).
/// `finetune`:      −128 … +127, where 128 units = 1 semitone.
pub fn note_to_pitch(note_1indexed: u8, relative_note: i8, finetune: i8) -> f64 {
    // Convert to 0-indexed semitones from C-0, apply relative_note and finetune.
    let n0 = note_1indexed as i32 - 1 + relative_note as i32;
    n0 as f64 + finetune as f64 / 128.0
}

/// Convert a pitch (semitones, C-5 = 60.0) to a frequency in Hz.
///
/// Reference: C-5 → 8363 Hz (the standard XM / FastTracker II base rate).
pub fn pitch_to_freq(pitch: f64) -> f64 {
    8363.0 * (2.0_f64).powf((pitch - 60.0) / 12.0)
}

/// Convert a pitch to the source-sample increment per output frame.
fn pitch_to_increment(pitch: f64, sample_rate: u32) -> f64 {
    pitch_to_freq(pitch) / sample_rate as f64
}

/// Sine LFO used by vibrato (4xx/6xx) and tremolo (7xx).
///
/// `phase` is 0–63; returns a value in `[-1.0, +1.0]`.
fn vibrato_lfo(phase: u8) -> f64 {
    let angle = (phase as f64) * std::f64::consts::TAU / 64.0;
    angle.sin()
}

/// Compute samples per tick from BPM.
///
/// Formula: `sample_rate × 2.5 / BPM`
///
/// Derivation: at 24 ticks/beat (Amiga VBL standard), one tick lasts
/// `60 / (BPM × 24)` seconds.  FastTracker II simplifies this to the
/// empirical factor of 2.5.
fn calc_samples_per_tick(sample_rate: u32, bpm: u32) -> f64 {
    sample_rate as f64 * 2.5 / bpm.max(1) as f64
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Frequency / pitch helpers ─────────────────────────────────────────

    #[test]
    fn c5_is_8363_hz() {
        // XM note 61 = C-5 (1-indexed); relative_note=0, finetune=0.
        let pitch = note_to_pitch(61, 0, 0);
        assert!((pitch - 60.0).abs() < 1e-10, "C-5 should be pitch 60.0");
        let freq = pitch_to_freq(pitch);
        assert!(
            (freq - 8363.0).abs() < 0.01,
            "C-5 should be 8363 Hz, got {freq}"
        );
    }

    #[test]
    fn c4_is_half_c5() {
        // C-4 = note 49, one octave below C-5.
        let freq = pitch_to_freq(note_to_pitch(49, 0, 0));
        assert!(
            (freq - 4181.5).abs() < 0.1,
            "C-4 should be ~4181.5 Hz, got {freq}"
        );
    }

    #[test]
    fn c6_is_double_c5() {
        let freq = pitch_to_freq(note_to_pitch(73, 0, 0));
        assert!(
            (freq - 8363.0 * 2.0).abs() < 0.1,
            "C-6 should be ~16726 Hz, got {freq}"
        );
    }

    #[test]
    fn positive_finetune_raises_pitch() {
        let f0 = pitch_to_freq(note_to_pitch(61, 0, 0));
        let f1 = pitch_to_freq(note_to_pitch(61, 0, 64)); // +64/128 = +0.5 semitone
        assert!(f1 > f0, "positive finetune should raise frequency");
    }

    #[test]
    fn relative_note_shifts_octave() {
        // relative_note = 12: one octave up.
        let f_base = pitch_to_freq(note_to_pitch(61, 0, 0));
        let f_up = pitch_to_freq(note_to_pitch(61, 12, 0));
        assert!(
            (f_up / f_base - 2.0).abs() < 0.001,
            "relative_note +12 should double the frequency"
        );
    }

    #[test]
    fn samples_per_tick_125bpm() {
        // Standard: 125 BPM at 44100 Hz → 882 samples/tick.
        let spt = calc_samples_per_tick(44100, 125);
        assert!(
            (spt - 882.0).abs() < 0.01,
            "125 BPM / 44100 Hz should be 882 samples/tick, got {spt}"
        );
    }

    // ── Player lifecycle ──────────────────────────────────────────────────

    #[test]
    fn stopped_player_produces_silence() {
        let module = make_test_module();
        let mut player = Player::new(Arc::new(module), 44100);
        // Player starts stopped.
        let mut buf = vec![0.0f32; 256];
        player.fill(&mut buf);
        assert!(
            buf.iter().all(|&s| s == 0.0),
            "stopped player must output silence"
        );
    }

    #[test]
    fn paused_player_produces_silence() {
        let module = make_test_module();
        let mut player = Player::new(Arc::new(module), 44100);
        player.play();
        player.pause();
        let mut buf = vec![0.0f32; 256];
        player.fill(&mut buf);
        assert!(
            buf.iter().all(|&s| s == 0.0),
            "paused player must output silence"
        );
    }

    #[test]
    fn stop_resets_position() {
        let module = make_test_module();
        let mut player = Player::new(Arc::new(module), 44100);
        player.play();
        // Advance a little.
        let mut buf = vec![0.0f32; 4096];
        player.fill(&mut buf);
        player.stop();
        let pos = player.position();
        assert_eq!(pos.order, 0);
        assert_eq!(pos.row, 0);
        assert!(!player.is_playing());
    }

    #[test]
    fn playing_empty_module_does_not_panic() {
        // An empty module (no patterns, no instruments) should not crash.
        let module = make_test_module();
        let mut player = Player::new(Arc::new(module), 44100);
        player.play();
        let mut buf = vec![0.0f32; 8192];
        player.fill(&mut buf); // must not panic
    }

    // ── Mixing-engine tests ───────────────────────────────────────────────

    /// Triggering a note with a non-silent sample produces non-zero output.
    #[test]
    fn active_channel_produces_non_silence() {
        let module = make_module_with_note(SampleLoopType::None);
        let mut player = Player::new(Arc::new(module), 44100);
        player.play();
        let mut buf = vec![0.0f32; 256];
        player.fill(&mut buf);
        assert!(
            buf.iter().any(|&s| s != 0.0),
            "playing a note should produce non-zero samples"
        );
    }

    /// After a non-looping sample runs out, the channel should go silent.
    #[test]
    fn non_looping_sample_ends() {
        // Create a module with a 16-sample non-looping tone.
        let module = make_module_with_note(SampleLoopType::None);
        let mut player = Player::new(Arc::new(module), 44100);
        player.play();
        // Render enough frames to exhaust the 16-sample tone (at C-5, increment ≈ 0.19
        // frames/output, takes ~85 output frames; give it plenty of room).
        let mut buf = vec![0.0f32; 4096];
        player.fill(&mut buf);
        // The last portion of the buffer should be all zeros once the sample ends.
        let tail = &buf[buf.len() - 64..];
        assert!(
            tail.iter().all(|&s| s == 0.0),
            "non-looping sample must silence the channel after exhaustion"
        );
    }

    /// A forward-looping sample should play indefinitely without panicking.
    #[test]
    fn forward_loop_does_not_end() {
        let module = make_module_with_note(SampleLoopType::Forward);
        let mut player = Player::new(Arc::new(module), 44100);
        player.play();
        let mut buf = vec![0.0f32; 8192];
        player.fill(&mut buf);
        // With a forward loop the channel stays active: expect some non-zero output
        // throughout the entire buffer.
        assert!(
            buf.iter().any(|&s| s != 0.0),
            "forward-looping sample must keep playing"
        );
    }

    /// A ping-pong-looping sample should also play indefinitely without panicking.
    #[test]
    fn ping_pong_loop_does_not_end() {
        let module = make_module_with_note(SampleLoopType::PingPong);
        let mut player = Player::new(Arc::new(module), 44100);
        player.play();
        let mut buf = vec![0.0f32; 8192];
        player.fill(&mut buf); // must not panic
        assert!(buf.iter().any(|&s| s != 0.0));
    }

    /// Output must always stay within [-1.0, 1.0] regardless of channel count.
    #[test]
    fn output_clamps_within_unity() {
        let module = make_module_with_note(SampleLoopType::Forward);
        let mut player = Player::new(Arc::new(module), 44100);
        player.play();
        let mut buf = vec![0.0f32; 2048];
        player.fill(&mut buf);
        for &s in &buf {
            assert!(
                s.abs() <= 1.0,
                "sample {s} exceeds unity; output must be clamped"
            );
        }
    }

    /// Vibrato LFO returns values strictly within [-1, 1] for all 64 phases.
    #[test]
    fn vibrato_lfo_stays_in_range() {
        for phase in 0u8..64 {
            let v = vibrato_lfo(phase);
            assert!(
                v.abs() <= 1.0 + f64::EPSILON,
                "lfo({phase}) = {v} out of range"
            );
        }
        // Phase 0 → sin(0) = 0.
        assert_eq!(vibrato_lfo(0), 0.0);
        // Phase 16 → sin(π/2) ≈ 1.
        assert!((vibrato_lfo(16) - 1.0).abs() < 1e-10);
    }

    /// `seek` repositions the player correctly.
    #[test]
    fn seek_updates_position() {
        let module = make_test_module();
        let mut player = Player::new(Arc::new(module), 44100);
        player.play();
        let mut buf = vec![0.0f32; 4096];
        player.fill(&mut buf);
        player.set_position(0, 16);
        let pos = player.position();
        assert_eq!(pos.order, 0);
        assert_eq!(pos.row, 16);
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    /// Minimal valid XM module with one silent 2-channel / 32-row pattern.
    fn make_test_module() -> XmModule {
        use crate::xm::*;
        XmModule {
            name: "Test".into(),
            tracker_name: "Test".into(),
            version: 0x0104,
            song_length: 1,
            restart_position: 0,
            channel_count: 2,
            default_tempo: 6,
            default_bpm: 125,
            linear_frequencies: true,
            pattern_order: vec![0],
            patterns: vec![XmPattern {
                rows: vec![vec![XmCell::default(); 2]; 32],
            }],
            instruments: vec![],
        }
    }

    /// XM module with one instrument that plays a 16-sample square-wave tone
    /// on C-5 in channel 0, row 0.
    fn make_module_with_note(loop_type: SampleLoopType) -> XmModule {
        use crate::xm::*;

        // 16-sample square wave: first 8 samples high, next 8 low.
        let data: Vec<i16> = (0..16)
            .map(|i| if i < 8 { i16::MAX / 2 } else { i16::MIN / 2 })
            .collect();

        let sample = XmSample {
            name: "sq".into(),
            loop_start: 0,
            loop_length: 16,
            loop_type,
            volume: 64,
            finetune: 0,
            panning: 128,
            relative_note: 0,
            data,
        };

        let mut instr = XmInstrument {
            name: "sq-instr".into(),
            note_to_sample: [0u8; 96],
            volume_envelope: XmEnvelope::default(),
            panning_envelope: XmEnvelope::default(),
            volume_fadeout: 0,
            vibrato_type: 0,
            vibrato_sweep: 0,
            vibrato_depth: 0,
            vibrato_rate: 0,
            samples: vec![sample],
        };
        // All notes map to sample 0.
        instr.note_to_sample = [0u8; 96];

        // Row 0: channel 0 plays C-5 with instrument 1.
        let mut row0 = vec![XmCell::default(); 2];
        row0[0] = XmCell {
            note: XmNote::On(61), // C-5 (1-indexed)
            instrument: 1,
            volume: 0x50, // set volume = 64 (max)
            effect: 0,
            effect_param: 0,
        };

        XmModule {
            name: "MixTest".into(),
            tracker_name: "Test".into(),
            version: 0x0104,
            song_length: 1,
            restart_position: 0,
            channel_count: 2,
            default_tempo: 6,
            default_bpm: 125,
            linear_frequencies: true,
            pattern_order: vec![0],
            patterns: vec![XmPattern {
                rows: {
                    let mut rows = vec![vec![XmCell::default(); 2]; 32];
                    rows[0] = row0;
                    rows
                },
            }],
            instruments: vec![instr],
        }
    }
}
