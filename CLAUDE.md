# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SoymilkyTracker is a music tracker application inspired by MilkyTracker and ProTracker, targeting both web (WebAssembly) and native desktop platforms from a single Rust codebase. It uses a pixel-art retro aesthetic while providing a more user-friendly experience than traditional module trackers (no need to memorize control commands).

## Technology Stack (Finalized 2026-03-28)

### Client (dual-target: Web WASM + Native Desktop)
- **UI framework**: [`egui`](https://github.com/emilk/egui) + [`eframe`](https://github.com/emilk/egui) — supports both native (OpenGL/wgpu) and WASM web targets out of the box
- **SF2 synthesis**: [`oxisynth`](https://github.com/PolyMeilex/OxiSynth) — pure Rust SF2 player, works on all targets
- **DSP / audio graph**: [`fundsp`](https://github.com/SamiPerttu/fundsp) — pure Rust, works on all targets
- **Module format**: XM (Extended Module) as primary; MOD for legacy compatibility
- **Instrument formats**: SF2 / SF3 (via `oxisynth`); GUS patch (`.pat`, Gravis UltraSound / Freepats project format)
- **Instruments**: Three vendored soundfonts in `assets/soundfonts/` — `TimGM6mb.sf2` (compact GM default, GPL-2.0+), `MuseScore_General.sf3` (high-quality GM, MIT, loaded via oxisynth `sf3` feature), `Open8bitVChiptuner.sf2` (chiptune palette, CC BY-SA 4.0); user-uploadable SF2/SF3 files also planned

#### Audio I/O — abstracted per target
The audio engine logic (DSP, mixing, synthesis) is shared. Only the I/O layer differs per target, behind an `AudioBackend` trait:

| Target | Backend | Crate |
|---|---|---|
| Web (WASM) | Web Audio `AudioWorklet` | [`wasm-bindgen`](https://github.com/rustwasm/wasm-bindgen) |
| Native desktop | Cross-platform audio I/O | [`cpal`](https://github.com/RustAudio/cpal) |

Use `#[cfg(target_arch = "wasm32")]` to gate platform-specific code.

`FillCallback` is cfg-gated: `Box<dyn FnMut(&mut [f32]) + Send + 'static>` on native (audio runs on a cpal thread); `Box<dyn FnMut(&mut [f32]) + 'static>` on WASM (single-threaded, no `Send` required).

**Key modules in `tracker-engine`:**
- `backend::NativeAudioBackend` / `WasmAudioBackend` — platform I/O; `preferred_sample_rate()` queries cpal on native, returns 44 100 on WASM
- `xm` — XM module file parser (FastTracker II format, v0x0104/0x0103)
- `player::Player` — XM channel mixing and sample playback engine; linear-frequency pitch model, forward/ping-pong looping, volume/panning envelopes with fadeout, full effect set (0x00–0x0F + Exx extended effects, vibrato LFO, tremolo, pattern loop, note cut/delay)
- `audio::TrackerAudio` — high-level transport controller; cfg-gated player handle (`Arc<Mutex<Player>>` native, `Rc<RefCell<Player>>` WASM); `load()`, `play()`, `pause()`, `stop()`, `seek()`, `position()`, `is_playing()`
- `synth::SfSynth` / `BundledFont` — oxisynth wrapper; `load_bundled()`, `load_font_bytes()`, `note_on/off()`, `program_change()`, `all_notes_off()`, `fill()`; `Open8bitVChiptuner.sf2` always embedded, `TimGM6mb.sf2` native-only
- `modfile` — ProTracker MOD parser (4–32 channels; `M.K.`, `M!K!`, `FLT4/8`, `OCTA`, `NNCHNu`); Amiga PAL period → XM pitch via `freq = 3_546_895 / period`
- `s3m` — Scream Tracker 3 `.s3m` parser; SCRM-magic detection; PCM type-1 instruments; c2spd → relative_note + finetune via `12×log₂(c2spd / 8363)`; packed 64-row pattern format; effect mapping A–Z → XM 0x00–0x0F + Exx; AdLib/OPL channels silently ignored
- `gus` — Gravis UltraSound `.pat` loader; pitch-correction formula `12×log₂(sample_rate / root_freq_hz)`; 96-entry note-to-sample map; tested against Freepats project patches

**UI font (`tracker-client`):**
`crates/tracker-client/src/app.rs` — `install_fonts()` registers `Ac437_IBM_EGA_8x8.ttf` under
the egui family name `"tracker"` (constant `FONT_TRACKER`) and sets it as the default for both
Proportional and Monospace families. Call once in `TrackerApp::new`. Use
`FontId::new(8.0, FontFamily::Name("tracker".into()))` for pixel-exact 8 px rendering.

**Pattern editor widget (`tracker-client`):**
`crates/tracker-client/src/pattern_editor.rs` — `PatternEditor` is the main egui widget.

- `PatternEditor::show(&mut self, ui, &mut XmPattern)` — call each frame; processes input then paints the grid. Takes `&mut XmPattern` so keyboard entry can write cells in-place before rendering.
- `PatternEditor` fields: `cursor_row`, `cursor_channel`, `cursor_col: SubCol`, `record_mode`, `octave: u8` (default 4), `step: usize` (default 1).
- `SubCol` enum — one of `Note | InsHi | InsLo | VolHi | VolLo | FxLtr | OpHi | OpLo`; drives cursor position, width, and key dispatch.
- **Keyboard entry**: QWERTY piano layout (MilkyTracker convention); `qwerty_to_note(key, octave)` maps Z-row = base octave, Q-row = octave+1, upper overflow = octave+2. `key_to_hex_nibble(key)` maps 0–9 / A–F for instrument/volume/effect columns. `Num1` = key-off (`XmNote::Off`), `Delete` = clear cell. Cursor auto-advances by `step` rows after entry.
- XM notes are **1-indexed**: `note = octave * 12 + semitone + 1` (range 1–96).
- `FontFamily::Name("tracker")` must be explicitly registered in the `families` map (not just `font_data`) — `install_fonts()` handles this; omitting it causes a runtime panic.

### Backend
- **Framework**: Rust + [`axum`](https://github.com/tokio-rs/axum)
- **Database**: PostgreSQL (user data, composition metadata, playlists)
- **File storage**: Local filesystem — designed for self-hosted, single server instance deployment (no object storage dependency). Serve files via `tower-http`'s `ServeDir`. Deployment docs should cover backup strategy for the storage directory.
- **Auth**: JWT or session-based; OAuth (GitHub/Google) for social login

## Repository Structure

```
crates/
  tracker-types/    # Shared data types (API DTOs, composition format) — no I/O, no async
  tracker-engine/   # Audio DSP, synthesis, AudioBackend trait, XM/MOD/GUS parsers, Player — compiles to WASM + native
  tracker-client/   # egui/eframe UI — compiles to WASM + native desktop
  tracker-server/   # Axum HTTP server — native only
assets/
  soundfonts/
    TimGM6mb.sf2              # General MIDI, GPL-2.0+ (bundled default instrument set)
    MuseScore_General.sf3     # Full GM + GS, MIT (high-quality, lazy-load candidate)
    Open8bitVChiptuner.sf2    # Chiptune / 8-bit style, CC BY-SA 4.0
    ATTRIBUTION               # License and attribution for all vendored soundfonts
  fonts/
    Ac437_IBM_EGA_8x8.ttf    # IBM EGA 8×8 bitmap font, CC BY 4.0 (VileR / int10h.org)
    ATTRIBUTION               # License and attribution for all vendored fonts
```

Dependency graph: `tracker-types` ← `tracker-engine` ← `tracker-client`; `tracker-types` ← `tracker-server`.

The `doc/` directory contains the product vision:
- `doc/product_design.md` — feature list, technology stack, UI/UX guidelines
- `doc/TODOs.md` — phased task list
- `doc/ui-mockups.md` — pixel-art UI wireframes and egui Painter implementation notes

## Architecture Intent

The application has two main layers:

1. **Client**: Single Rust codebase targeting both WASM (web) and native desktop. On WASM, the `egui`/`eframe` UI and audio fill logic both run on the main WASM thread. Audio samples are pre-rendered via a `requestAnimationFrame` loop, then posted as `Float32Array` chunks to a `TrackerProcessor` `AudioWorklet` via `MessagePort`. On native, `cpal` drives audio I/O directly via a `FillCallback`. The server connection is optional for native — the app can operate fully offline.
2. **Server**: Rust/Axum REST API backed by PostgreSQL and local filesystem storage. Stores user compositions and profiles; serves published works and playlists. Designed for self-hosted, single server instance deployment.

## Build & Development Commands

### Prerequisites

```bash
rustup show          # installs toolchain + wasm32 target from rust-toolchain.toml
cargo install trunk  # WASM web dev server (install once)
```

### Native desktop

```bash
cargo check                    # check all workspace crates
cargo test --all               # run all tests
cargo run -p tracker-client    # launch the tracker UI (native window)
cargo run -p tracker-server    # launch the backend server
```

### Web (WASM) — via Trunk

```bash
cd crates/tracker-client
trunk serve                    # dev server at http://localhost:8080 (hot-reload)
trunk build --release          # production WASM bundle → dist/
```

### Linting

```bash
cargo fmt --all --check                        # check formatting
cargo clippy --all-targets -- -D warnings      # lint (warnings are errors)
```

### CI

GitHub Actions (`.github/workflows/ci.yml`) runs on every push/PR to `main`:
- **fmt** — `cargo fmt --all --check`
- **native** — `cargo clippy --all-targets` + `cargo test --all`
- **wasm** — `cargo check --target wasm32-unknown-unknown` for `tracker-types`, `tracker-engine`, `tracker-client`

### PoC spikes (standalone — not part of the workspace)

```bash
# Native
cargo run                  # in poc/cpal-native/

# Web (WASM)
trunk serve                # in poc/wasm-audio/, poc/egui-grid/, poc/oxisynth-wasm/
```

## While Coding

- When coding, provide sufficient comments to help other developers understand the logic.
- **Rust** — `rustfmt` runs automatically on every `*.rs` file after each write or edit. `cargo clippy` must also pass clean.

## After Every Change

1. Update all relevant documentation
2. Add essential but missing tests to improve test coverage and ensure code quality
3. check if there is any missing or incomplete test
4. Remove the finishied tasks from TODOs
5. When a bug is discovered, **always** check for similar issues across the project after applying the fix


## License

GNU General Public License v3.0 or later (GPL-3.0-or-later). All new source files should include the following SPDX header:

```
// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later
```
