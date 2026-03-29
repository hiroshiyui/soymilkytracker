<!--
 SPDX-FileCopyrightText: 2026 HUIHONG YOU
 SPDX-License-Identifier: GPL-3.0-or-later
-->

# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Phase 2 — Tracker Editor UI (in progress)

#### Done (2026-03-29)

- **Pattern editor grid** (`tracker-client::pattern_editor`)
  - `PatternEditor` egui widget — 87 px per channel: Note (3 chars) + Inst Hi/Lo + Vol Hi/Lo + Fx letter + Op Hi/Lo, with 1 px separators
  - 8 px row height matching IBM EGA 8×8 font; 16 px row-number margin; 12 px channel header row
  - MilkyTracker classic colour palette: per-field semantic colours, beat-row highlighting (every 4th / 8th row), cursor-row band and cursor-cell overlay
  - Bidirectional `ScrollArea`; scroll-to-cursor on every keyboard navigation move
  - Navigation: arrow keys (sub-column and row), Tab/Shift-Tab (channel), Home/End, Page Up/Down
  - Click-to-position cursor with sub-column detection
  - `SubCol` enum: `Note | InsHi | InsLo | VolHi | VolLo | FxLtr | OpHi | OpLo`

- **QWERTY piano keyboard note entry** (`tracker-client::pattern_editor`)
  - `qwerty_to_note(key, octave)` — MilkyTracker layout: Z-row = base octave, Q-row = octave+1, upper overflow = octave+2; clamps to XM range 1–96
  - `key_to_hex_nibble(key)` — Num0–Num9 and A–F for instrument / volume / effect columns; cursor auto-advances through sub-columns on each digit
  - `Num1` = key-off (`XmNote::Off`); `Delete` = clear cell; cursor advances by configurable `step` after note entry
  - `PatternEditor` fields: `octave: u8` (default 4), `step: usize` (default 1); status bar shows current `Oct` and `Stp`
  - 17 unit tests total in `tracker-client`

#### Planned

- Instrument list panel
- Song arranger / order list
- Sample waveform viewer and basic editor
- Transport controls (play song, play pattern, record mode)
- BPM / tempo / speed controls
- Undo/redo history
- Custom instrument file upload (SF2, SF3, GUS `.pat`)
- Keyboard shortcut overlay / help panel

---

## Phase 1 — Core Audio Engine (2026-03-29)

### Added

- **XM channel mixing and sample playback engine** (`tracker-engine::player`)
  - Linear-frequency pitch model (`8363 × 2^((pitch−60)/12)`)
  - Forward and ping-pong sample looping
  - Volume and panning envelopes with sustain, loop, and fadeout
  - Full XM effect set: arpeggio (0xx), portamento up/down (1xx/2xx), tone portamento (3xx), vibrato (4xx), combined vol+porta/vibrato (5xx/6xx), tremolo (7xx), panning (8xx), sample offset (9xx), volume slide (Axx), order jump (Bxx), set volume (Cxx), pattern break (Dxx), set speed/BPM (Fxx)
  - Extended effects (Exx): fine portamento (E1x/E2x), pattern loop (E6x), panning (E8x), retrigger (E9x), fine vol slide (EAx/EBx), note cut (ECx), note delay (EDx)
  - Volume column effects (fine slide, vibrato speed/depth, panning)
  - 45 unit tests

- **`TrackerAudio` transport controller** (`tracker-engine::audio`)
  - High-level API: `load()`, `play()`, `pause()`, `stop()`, `seek()`, `position()`, `is_playing()`
  - cfg-gated player handle: `Arc<Mutex<Player>>` on native (cpal thread-safe), `Rc<RefCell<Player>>` on WASM (single-threaded)
  - `preferred_sample_rate()` on `AudioBackend` trait — queries cpal device on native, returns 44 100 on WASM

- **`SfSynth` SF2/SF3 synthesiser** (`tracker-engine::synth`)
  - Wraps `oxisynth`; stereo-interleaved fill compatible with `FillCallback`
  - `BundledFont` enum: `Open8bitVChiptuner` (92 KB, always embedded), `TimGm6mb` (5.7 MB, native-only)
  - MIDI event dispatch: `note_on`, `note_off`, `program_change`, `all_notes_off`
  - `load_font_bytes()` for user-supplied SF2/SF3 on any target

- **MOD file parser** (`tracker-engine::modfile`)
  - Supports 4-to-32 channel ProTracker/compatible files
  - Format variants: `M.K.`, `M!K!`, `FLT4`, `FLT8`, `OCTA`, `CD81`, `NNCHNu` (`2CHN`–`32CH`)
  - Amiga PAL period → XM pitch: `freq = 3_546_895 / period`; `pitch = 60 + 12×log₂(freq/8363)`
  - 4-bit finetune nibble → XM i8 finetune (×16 scale factor)
  - Produces `XmModule` — fully playable by the existing `Player`

- **GUS `.pat` patch file loader** (`tracker-engine::gus`)
  - Parses Gravis UltraSound / Freepats project `.pat` files
  - Pitch-correction formula: `relative_note + finetune/128 = 12×log₂(sample_rate / root_freq_hz)`
  - 96-entry note-to-sample map built from `[low_freq, high_freq]` millihertz ranges
  - 8-bit and 16-bit, signed and unsigned PCM; forward and ping-pong looping
  - Produces `XmInstrument` — loadable by the existing `Player`
  - Verified against Freepats patches at `/usr/share/midi/freepats/`

---

## Phase 0 — Technology Decisions & Project Setup (2026-03-28)

### Added

- Rust Cargo workspace with four crates: `tracker-types`, `tracker-engine`, `tracker-client`, `tracker-server`
- `rust-toolchain.toml` pinning stable Rust + `wasm32-unknown-unknown` target
- GitHub Actions CI: fmt check, clippy (`-D warnings`), native tests, WASM `cargo check`
- **`AudioBackend` trait** with cfg-gated `FillCallback` (`Send` on native, not on WASM)
  - `NativeAudioBackend` — cpal stereo interleaved output stream
  - `WasmAudioBackend` — Web Audio `AudioWorklet` via `MessagePort` + `requestAnimationFrame` pre-render loop
- **XM module file parser** (`tracker-engine::xm`) — FastTracker II format v0x0104/0x0103; compressed pattern data; delta-decoded samples; envelopes; variable-length headers
- Three vendored soundfonts in `assets/soundfonts/`: `TimGM6mb.sf2` (GPL-2.0+), `MuseScore_General.sf3` (MIT, oxisynth `sf3` feature), `Open8bitVChiptuner.sf2` (CC BY-SA 4.0)
- IBM EGA 8×8 bitmap font (`assets/fonts/Ac437_IBM_EGA_8x8.ttf`, CC BY 4.0) registered in egui as primary UI typeface
- Pixel-art UI mockups (`doc/ui-mockups.md`) — MilkyTracker-faithful wireframes, colour palette, egui Painter notes
- Four confirmed PoC spikes: WASM AudioWorklet, cpal native audio, egui tracker grid, oxisynth SF2 synthesis
- `egui`/`eframe` UI skeleton (`tracker-client`) compiling to both WASM and native desktop
