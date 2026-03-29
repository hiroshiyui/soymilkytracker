# SoymilkyTracker

> **Work in progress — not ready for use.**

A music tracker inspired by [MilkyTracker](https://milkytracker.org/) and ProTracker, built entirely in Rust. It targets both **web** (WebAssembly) and **native desktop** from a single codebase, bringing the classic module tracker experience to the browser and desktop with a pixel-art retro aesthetic — without requiring users to memorize arcane keyboard commands.

## Features

- **Pattern editor** — compose music in the classic tracker grid (XM format primary, MOD legacy support)
- **Built-in soundset** — [TimGM6mb](https://packages.debian.org/sid/timgm6mb-soundfont) (compact GM, GPL-2.0+), [MuseScore General](https://ftp.osuosl.org/pub/musescore/soundfont/MuseScore_General/) (full GM+GS, MIT), and [Open8bitVChiptuner](https://codeberg.org/trzyglow/Open8bitVChiptuner) (chiptune palette, CC BY-SA 4.0) soundfonts bundled
- **Custom instruments** — upload and use your own SF2, SF3, or GUS patch (`.pat`) files
- **Cloud save** — save and download compositions from a remote server
- **Publishing** — publish works and playlists; share via an embedded WASM player
- **User profiles** — personal pages, community browsing, and discovery

## Technology Stack

### Client (Web WASM + Native Desktop)

| Layer | Library |
|---|---|
| UI framework | [`egui`](https://github.com/emilk/egui) + [`eframe`](https://github.com/emilk/egui) |
| DSP / audio graph | [`fundsp`](https://github.com/SamiPerttu/fundsp) |
| SF2 synthesis | [`oxisynth`](https://github.com/PolyMeilex/OxiSynth) |
| Audio I/O — Web | Web Audio `AudioWorklet` + [`wasm-bindgen`](https://github.com/rustwasm/wasm-bindgen) |
| Audio I/O — Native | [`cpal`](https://github.com/RustAudio/cpal) |

### Backend

| Layer | Choice |
|---|---|
| Framework | Rust + [`axum`](https://github.com/tokio-rs/axum) |
| Database | PostgreSQL |
| File storage | Local filesystem (self-hosted friendly) |
| Auth | JWT / OAuth (GitHub, Google) |

## Development Prerequisites

| Tool | Purpose | Install |
|---|---|---|
| Rust (stable) | Compiler | [rustup.rs](https://rustup.rs) — toolchain pinned via `rust-toolchain.toml` |
| `wasm32-unknown-unknown` | WASM compile target | Auto-installed by `rustup` from `rust-toolchain.toml` |
| [`trunk`](https://trunkrs.dev) | WASM web build & dev server | `cargo install trunk` |

## Project Status

Early development — **Phase 1 complete, Phase 2 underway**. Not yet usable as an application.

| Milestone | Status |
|---|---|
| Phase 0 — technology decisions, workspace, CI | **Done** |
| Phase 1 — core audio engine | **Done** |
| Phase 2 — Tracker Editor UI | **In progress** |
| Phase 3+ — file I/O, backend, community | Not started |

**What exists today:**
- `AudioBackend` trait with full implementations for both targets — `NativeAudioBackend` (cpal, stereo interleaved) and `WasmAudioBackend` (Web Audio `AudioWorklet` via MessagePort + requestAnimationFrame fill loop)
- XM module file parser (`tracker-engine::xm`) — compressed pattern data, delta-decoded samples, envelopes, variable-length headers
- XM channel mixing and sample playback engine (`tracker-engine::player`) — linear-frequency pitch model, forward/ping-pong looping, volume/panning envelopes with fadeout, full effect set (arpeggio, portamento, vibrato, tremolo, volume slide, fine slides, pattern loop, note cut/delay, sample offset, Exx extended effects); 45 unit tests
- `TrackerAudio` high-level transport controller — cfg-gated `Arc<Mutex<Player>>` (native) vs `Rc<RefCell<Player>>` (WASM); `load()`, `play()`, `pause()`, `stop()`, `seek()`, `position()`
- `SfSynth` SF2/SF3 synthesiser wrapping `oxisynth` — `load_bundled()`, `load_font_bytes()`, full MIDI event dispatch, stereo-interleaved fill
- MOD file parser (`tracker-engine::modfile`) — 4-to-32 channel ProTracker/compatible, Amiga PAL period → XM pitch conversion, all common format variants (`M.K.`, `M!K!`, `FLT4/8`, `OCTA`, `NNCHNu`)
- GUS `.pat` patch file loader (`tracker-engine::gus`) — Freepats-compatible; pitch-correction via `12×log₂(sample_rate/root_freq_hz)`; 96-entry note-to-sample map
- Three vendored soundfonts: `TimGM6mb.sf2` (GM, GPL-2.0+), `MuseScore_General.sf3` (full GM+GS, MIT), `Open8bitVChiptuner.sf2` (chiptune, CC BY-SA 4.0)
- IBM EGA 8×8 bitmap font (`assets/fonts/Ac437_IBM_EGA_8x8.ttf`, CC BY 4.0) vendored and registered in egui as the primary UI typeface
- Pixel-art UI mockups (`doc/ui-mockups.md`) — MilkyTracker-faithful wireframes, colour palette, and egui Painter implementation notes for all panels
- **Pattern editor grid** (`tracker-client::pattern_editor`) — `PatternEditor` egui widget with the full MilkyTracker classic layout: 87 px/channel (Note + Inst + Vol + Fx), per-field semantic colours, beat-row highlighting, cursor-row band, bidirectional scrolling, click-to-position, arrow/Tab/Home/End/PgUp/PgDn keyboard navigation
- **QWERTY piano keyboard note entry** — MilkyTracker layout (Z-row = base octave, Q-row = octave+1, upper overflow = octave+2); `Num1` = key-off, `Delete` = clear cell; hex-digit entry for instrument/volume/effect; configurable octave and step; 17 unit tests
- Four confirmed proof-of-concept spikes (WASM AudioWorklet, egui tracker grid, oxisynth SF2 synthesis, cpal native audio)

Contributions and feedback are welcome, but expect significant breaking changes at any stage.

See [`doc/TODOs.md`](doc/TODOs.md) for the phased task list and [`doc/product_design.md`](doc/product_design.md) for the full product vision.

## Naming Origin

The author is lactose intolerant, so he chose a name that suits him — paying homage to and parodying MilkyTracker.

## License

Copyright 2026 HUIHONG YOU

SoymilkyTracker is free software: you can redistribute it and/or modify it under the terms of the [GNU General Public License](LICENSE) as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
