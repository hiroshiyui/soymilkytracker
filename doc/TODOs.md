<!--
 SPDX-FileCopyrightText: 2026 HUIHONG YOU
 SPDX-License-Identifier: GPL-3.0-or-later
-->

# TODOs

## Phase 0 — Technology Decision & Project Setup

- [x] Evaluate and finalize technology stack
  - **Decided**: Rust; `egui`/`eframe` UI (dual-target); `fundsp` + `oxisynth` audio; Axum + PostgreSQL backend
- [x] Choose backend framework → **Axum**
- [x] Choose database → **PostgreSQL** + local filesystem storage (self-hosted friendly)
- [x] Decide on compile targets → **Web (WASM) + Native Desktop** from a single codebase
- [x] Choose audio I/O strategy → `AudioBackend` trait: Web Audio `AudioWorklet` on WASM, `cpal` on native
- [x] Proof-of-concept spike: Rust WASM `AudioWorklet` producing sound in-browser
  - See `poc/wasm-audio/` — build with `trunk serve` from that directory
- [x] Proof-of-concept spike: `cpal` producing sound on native desktop
  - See `poc/cpal-native/` — `cargo run`
- [x] Proof-of-concept spike: `egui`/`eframe` rendering a tracker grid cell at pixel-art scale (web + native)
  - See `poc/egui-grid/` — `cargo run` for native, `trunk serve` for web
- [x] Proof-of-concept spike: `oxisynth` playing a Freepats note (both targets)
  - See `poc/oxisynth-wasm/` — `trunk serve` (requires `TimGM6mb.sf2` symlink)
- [x] Set up repository structure → Cargo workspace with `tracker-types`, `tracker-engine`, `tracker-client`, `tracker-server`
- [x] Configure build toolchain: `cargo` for native, `trunk` for WASM web
  - `rust-toolchain.toml` pins stable + `wasm32-unknown-unknown`; `trunk` serves `crates/tracker-client/`
- [x] Set up CI pipeline (build + test for both targets)
  - See `.github/workflows/ci.yml`: fmt, native (clippy + test), wasm check
- [x] Update CLAUDE.md with build commands

## Phase 1 — Core Audio Engine

- [x] Research and select audio I/O backends → Web Audio `AudioWorklet` (WASM) + `cpal` (native)
- [x] Define `AudioBackend` trait and implement for both targets
  - `WasmAudioBackend` — Web Audio `AudioWorklet` via `wasm-bindgen`
  - `NativeAudioBackend` — `cpal`
- [x] Implement XM module file parser (primary format, as used by MilkyTracker)
  - See `crates/tracker-engine/src/xm.rs` — parses XM v0x0104/0x0103; handles compressed pattern cells, delta-decoded samples, envelopes
- [ ] Implement MOD module file parser (legacy compatibility)
- [ ] Implement GUS patch (`.pat`) file loader
  - Format used by Gravis UltraSound and the [Freepats project](http://freepats.zenvoid.org/); historically significant in the DOS/tracker era
  - Allows loading individual instrument samples from `.pat` collections alongside SF2/SF3
- [ ] Implement channel mixing and sample playback engine
- [ ] Support basic tracker effects (volume, pitch, arpeggio, portamento, vibrato, etc.)
- [ ] Integrate bundled soundfonts as the default instrument set via `oxisynth`
  - `TimGM6mb.sf2` (GPL-2.0+) — compact GM default
  - `MuseScore_General.sf3` (MIT) — high-quality GM, opt-in / lazy-load
  - `Open8bitVChiptuner.sf2` (CC BY-SA 4.0) — chiptune palette
  - All three vendored in `assets/soundfonts/`; oxisynth `sf3` feature already enabled
- [ ] Expose play / pause / stop / seek controls through the `AudioBackend` trait
- [ ] Write unit tests for parser and mixing engine

## Phase 2 — Tracker Editor UI

- [ ] Design pixel-art UI mockups (pattern editor, instrument list, sample editor, song arranger)
- [ ] Implement pattern editor grid (note, instrument, volume, effect columns per channel)
- [ ] Implement keyboard input mapping for note entry (piano-key layout on QWERTY)
- [ ] Implement instrument list panel
- [ ] Implement song arranger / order list
- [ ] Implement sample waveform viewer and basic editor (loop points, trim)
- [ ] Implement transport controls (play song, play pattern, record mode)
- [ ] Implement BPM / tempo / speed controls
- [ ] Implement undo/redo history
- [ ] Support custom instrument file upload and hot-loading into the instrument list (SF2, SF3, GUS `.pat`)
- [ ] Implement keyboard shortcut overlay / help panel (to eliminate need to memorize commands)

## Phase 3 — File I/O & Local Storage

- [ ] Export composition to XM file format for download / save to disk
- [ ] Import existing XM/MOD files from local disk or file picker
- [ ] Auto-save draft: `localStorage` / `IndexedDB` on web; local file on native
- [ ] Export rendered audio as WAV or MP3
  - Web: via Web Audio API
  - Native: via `cpal` or a Rust encoding crate

## Phase 4 — Backend & User Accounts

- [ ] Design REST API schema: users, compositions, playlists
- [ ] Implement user registration, login, and session management (JWT or cookie-based)
- [ ] OAuth login (GitHub, Google)
- [ ] Implement composition CRUD: save draft, publish, unpublish, delete
- [ ] Implement local filesystem storage for composition files and custom soundfonts
  - Serve files via `tower-http` `ServeDir`
  - Document backup strategy for the storage directory
- [ ] Implement playlist CRUD (create, add/remove tracks, reorder, publish)
- [ ] Implement user profile (avatar, bio, published works)
- [ ] Set up database migrations
- [ ] Native app: make server connection optional (offline-capable by default)

## Phase 5 — Community & Discovery

- [ ] Public work listing / browse page
- [ ] Search by title, author, or tag
- [ ] Tagging system for compositions
- [ ] Like / bookmark a composition
- [ ] Creator profile public page

## Phase 6 — Embedded Player

- [ ] Build a standalone lightweight WASM player (no editor UI) for embedding published works
- [ ] Playlist playback with track-to-track transitions
- [ ] Shareable player URL per composition / playlist
- [ ] Embed snippet (iframe) for third-party sites

## Phase 7 — Polish & Release

- [ ] Accessibility audit (keyboard navigation, screen reader hints where feasible)
- [ ] Responsive layout for various screen sizes (web)
- [ ] Performance profiling of audio engine on both targets (minimize dropouts)
- [ ] Cross-browser testing (Chrome, Firefox, Safari)
- [ ] Native desktop packaging (`.app`, `.exe`, `.deb` / `.AppImage`)
- [ ] Write user-facing documentation / tutorial for first-time tracker users
- [ ] Set up production deployment: containerized server + CDN for WASM/assets
- [ ] Security review of file upload pipeline
