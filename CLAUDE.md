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
- **Instruments**: Built-in Freepats sound library (SF2); user-uploadable SF2/WAV files

#### Audio I/O — abstracted per target
The audio engine logic (DSP, mixing, synthesis) is shared. Only the I/O layer differs per target, behind an `AudioBackend` trait:

| Target | Backend | Crate |
|---|---|---|
| Web (WASM) | Web Audio `AudioWorklet` | [`wasm-bindgen`](https://github.com/rustwasm/wasm-bindgen) |
| Native desktop | Cross-platform audio I/O | [`cpal`](https://github.com/RustAudio/cpal) |

Use `#[cfg(target_arch = "wasm32")]` to gate platform-specific code.

### Backend
- **Framework**: Rust + [`axum`](https://github.com/tokio-rs/axum)
- **Database**: PostgreSQL (user data, composition metadata, playlists)
- **File storage**: Local filesystem — designed for self-hosted, single server instance deployment (no object storage dependency). Serve files via `tower-http`'s `ServeDir`. Deployment docs should cover backup strategy for the storage directory.
- **Auth**: JWT or session-based; OAuth (GitHub/Google) for social login

## Repository Structure

```
crates/
  tracker-types/    # Shared data types (API DTOs, composition format) — no I/O, no async
  tracker-engine/   # Audio DSP, synthesis, AudioBackend trait — compiles to WASM + native
  tracker-client/   # egui/eframe UI — compiles to WASM + native desktop
  tracker-server/   # Axum HTTP server — native only
```

Dependency graph: `tracker-types` ← `tracker-engine` ← `tracker-client`; `tracker-types` ← `tracker-server`.

The `doc/` directory contains the product vision:
- `doc/product_design.md` — feature list, technology stack, UI/UX guidelines
- `doc/TODOs.md` — phased task list

## Architecture Intent

The application has two main layers:

1. **Client**: Single Rust codebase targeting both WASM (web) and native desktop. On WASM, the audio engine runs in a Web Audio `AudioWorklet` thread; the `egui`/`eframe` UI runs in the main WASM thread, communicating via `SharedArrayBuffer` / message passing. On native, `cpal` drives audio I/O directly. The server connection is optional for native — the app can operate fully offline.
2. **Server**: Rust/Axum REST API backed by PostgreSQL and local filesystem storage. Stores user compositions and profiles; serves published works and playlists. Designed for self-hosted, single server instance deployment.

## Build & Development Commands

No build system has been set up yet. Update this file when a toolchain is chosen and configured.

## License

GNU General Public License v3.0 or later (GPL-3.0-or-later). All new source files should include the following SPDX header:

```
// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later
```
