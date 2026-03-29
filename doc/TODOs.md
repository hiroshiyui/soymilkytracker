<!--
 SPDX-FileCopyrightText: 2026 HUIHONG YOU
 SPDX-License-Identifier: GPL-3.0-or-later
-->

# TODOs

## Phase 0 — Technology Decision & Project Setup ✓ Complete

Stack finalised (2026-03-28): Rust; `egui`/`eframe` UI (dual WASM + native); `fundsp` + `oxisynth`
audio; Axum + PostgreSQL backend. Four PoC spikes confirmed (WASM AudioWorklet, cpal native,
egui grid, oxisynth synthesis). Cargo workspace, `rust-toolchain.toml`, Trunk, and GitHub Actions
CI all in place.

## Phase 1 — Core Audio Engine ✓ Complete

All items done (2026-03-29): `AudioBackend` trait + `NativeAudioBackend` (cpal) +
`WasmAudioBackend` (AudioWorklet); XM parser; XM channel mixing engine with full effect set
(vibrato, tremolo, portamento, Exx extended effects, volume/panning envelopes, fadeout);
`TrackerAudio` transport controller; `SfSynth`/`BundledFont` oxisynth wrapper; MOD parser
(4–32 channels, Amiga PAL period conversion); GUS `.pat` loader (Freepats-compatible, pitch
correction, note-to-sample map). 45 unit tests passing.

## Phase 2 — Tracker Editor UI

Done (2026-03-29): pixel-art UI mockups (`doc/ui-mockups.md`); IBM EGA 8×8 font vendored and
registered in egui; pattern editor grid (`PatternEditor`, 87 px/channel, MilkyTracker colour
palette, ScrollArea, click-to-position, navigation keys); QWERTY piano keyboard entry
(`qwerty_to_note`, `key_to_hex_nibble`, key-off, clear cell, step advance). 17 unit tests.

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
