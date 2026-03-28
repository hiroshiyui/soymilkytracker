# TODOs

## Phase 0 — Technology Decision & Project Setup

- [x] Evaluate and finalize technology stack
  - **Decided**: Rust → WASM; `egui`/`eframe` UI; `fundsp` + `oxisynth` audio; Axum + PostgreSQL backend
- [x] Choose backend framework → **Axum**
- [x] Choose database → **PostgreSQL** + S3-compatible object storage
- [ ] Proof-of-concept spike: Rust WASM AudioWorklet producing sound in-browser
- [ ] Proof-of-concept spike: `egui` rendering a tracker grid cell at pixel-art scale
- [ ] Proof-of-concept spike: `oxisynth` playing a Freepats note from WASM
- [ ] Set up monorepo or split repo structure (client / server / shared)
- [ ] Configure build toolchain (CI pipeline, WASM build, asset pipeline)
- [ ] Write initial README.md
- [ ] Update CLAUDE.md with build commands once toolchain is configured

## Phase 1 — Core Audio Engine (WASM)

- [x] Research and select an audio rendering backend → **Web Audio API AudioWorklet** with `fundsp` + `oxisynth`
- [ ] Implement MOD/XM module file parser (support at least XM format as used by MilkyTracker)
- [ ] Implement channel mixing and sample playback engine
- [ ] Support basic tracker effects (volume, pitch, arpeggio, portamento, vibrato, etc.)
- [ ] Integrate Freepats General MIDI instrument library as built-in soundset
- [ ] Expose play / pause / stop / seek controls via WASM bindings
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
- [ ] Support custom sound font file upload and hot-loading into the instrument list
- [ ] Implement keyboard shortcut overlay / help panel (to eliminate need to memorize commands)

## Phase 3 — File I/O & Local Storage

- [ ] Export composition to XM (or native) file format for download
- [ ] Import existing XM/MOD files from local disk
- [ ] Auto-save draft to browser localStorage / IndexedDB
- [ ] Export rendered audio as WAV or MP3 via Web Audio API

## Phase 4 — Backend & User Accounts

- [ ] Design REST (or GraphQL) API schema: users, compositions, playlists
- [ ] Implement user registration, login, and session management (JWT or cookie-based)
- [ ] Implement composition CRUD: save draft, publish, unpublish, delete
- [ ] Implement file storage for composition files and custom soundfonts
- [ ] Implement playlist CRUD (create, add/remove tracks, reorder, publish)
- [ ] Implement user profile page (avatar, bio, published works)
- [ ] Set up database migrations

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
- [ ] Responsive layout for various screen sizes
- [ ] Performance profiling of WASM audio engine (minimize audio dropouts)
- [ ] Cross-browser testing (Chrome, Firefox, Safari)
- [ ] Write user-facing documentation / tutorial for first-time tracker users
- [ ] Set up production deployment (containerization, CDN for WASM/assets)
- [ ] Security review of file upload pipeline
