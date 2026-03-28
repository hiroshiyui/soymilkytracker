# SoymilkyTracker

> **Work in progress — not ready for use.**

A music tracker inspired by [MilkyTracker](https://milkytracker.org/) and ProTracker, built entirely in Rust. It targets both **web** (WebAssembly) and **native desktop** from a single codebase, bringing the classic module tracker experience to the browser and desktop with a pixel-art retro aesthetic — without requiring users to memorize arcane keyboard commands.

## Features

- **Pattern editor** — compose music in the classic tracker grid (XM format primary, MOD legacy support)
- **Built-in soundset** — [TimGM6mb](https://packages.debian.org/sid/timgm6mb-soundfont) (compact GM, GPL-2.0+) and [MuseScore General](https://ftp.osuosl.org/pub/musescore/soundfont/MuseScore_General/) (high-quality GM, MIT) soundfonts bundled; [Open8bitVChiptuner](https://codeberg.org/trzyglow/Open8bitVChiptuner) chiptune palette included for the retro aesthetic
- **Custom instruments** — upload and use your own SF2 / WAV soundfonts
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

Early development — **Phase 0 complete, Phase 1 underway**. Not yet usable as an application.

| Milestone | Status |
|---|---|
| Phase 0 — technology decisions, workspace, CI | **Done** |
| Phase 1 — core audio engine | **In progress** |
| Phase 2+ — UI, file I/O, backend, community | Not started |

**What exists today:**
- `AudioBackend` trait with full implementations for both targets — `NativeAudioBackend` (cpal, stereo interleaved) and `WasmAudioBackend` (Web Audio `AudioWorklet` via MessagePort + requestAnimationFrame fill loop)
- XM module file parser (`tracker-engine::xm`) — handles compressed pattern data, delta-decoded samples, envelopes, and variable-length headers
- Two vendored SF2 soundfonts: `TimGM6mb.sf2` (General MIDI, GPL-2.0+) and `Open8bitVChiptuner.sf2` (chiptune style, CC BY-SA 4.0)
- Four confirmed proof-of-concept spikes (WASM AudioWorklet, egui tracker grid, oxisynth SF2 synthesis, cpal native audio)

Contributions and feedback are welcome, but expect significant breaking changes at any stage.

See [`doc/TODOs.md`](doc/TODOs.md) for the phased task list and [`doc/product_design.md`](doc/product_design.md) for the full product vision.

## Naming Origin

The author is lactose intolerant, so he chose a name that suits him — paying homage to and parodying MilkyTracker.

## License

Copyright 2026 HUIHONG YOU

SoymilkyTracker is free software: you can redistribute it and/or modify it under the terms of the [GNU General Public License](LICENSE) as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
