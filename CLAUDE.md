# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SoymilkyTracker is a web-based music tracker application inspired by MilkyTracker and ProTracker. It targets a pixel-art retro aesthetic while providing a more user-friendly experience than traditional module trackers (no need to memorize control commands).

## Technology Stack (Finalized 2026-03-28)

### Client (WebAssembly)
- **Audio engine**: Rust → WASM, running inside a Web Audio `AudioWorklet` for low-latency playback
- **UI framework**: [`egui`](https://github.com/emilk/egui) + [`eframe`](https://github.com/emilk/egui) (immediate-mode GUI with WASM/web target)
- **SF2 synthesis**: [`oxisynth`](https://github.com/PolyMeilex/OxiSynth) — pure Rust SF2 player, WASM-compatible
- **DSP / audio graph**: [`fundsp`](https://github.com/SamiPerttu/fundsp) — Rust audio DSP library
- **WASM bindings**: [`wasm-bindgen`](https://github.com/rustwasm/wasm-bindgen)
- **Module format**: XM (Extended Module) as primary; MOD for legacy compatibility
- **Instruments**: Built-in Freepats sound library (SF2); user-uploadable SF2/WAV files

### Backend
- **Framework**: Rust + [`axum`](https://github.com/tokio-rs/axum)
- **Database**: PostgreSQL (user data, composition metadata, playlists)
- **File storage**: Local filesystem — designed for self-hosted, single server instance deployment (no object storage dependency). Serve files via `tower-http`'s `ServeDir`. Deployment docs should cover backup strategy for the storage directory.
- **Auth**: JWT or session-based; OAuth (GitHub/Google) for social login

## Project Status

This is a greenfield project — no source code exists yet. The `doc/` directory contains the product vision:

- `doc/product_design.md` — feature list, technology stack discussion, UI/UX guidelines
- `doc/TODOs.md` — task tracking (currently empty)

## Architecture Intent

The application has two main layers:

1. **Client (WebAssembly)**: Single Rust codebase compiled to WASM. The audio engine runs in a Web Audio `AudioWorklet` thread; the `egui`/`eframe` UI runs in the main WASM thread. Communication between them uses `SharedArrayBuffer` / message passing.
2. **Server**: Rust/Axum REST API backed by PostgreSQL and object storage. Stores user compositions and profiles; serves published works and playlists for WASM-based playback.

## Build & Development Commands

No build system has been set up yet. Update this file when a toolchain is chosen and configured.

## License

GNU General Public License v3.0 or later (GPL-3.0-or-later). All new source files should include the following SPDX header:

```
// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later
```
