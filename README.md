# SoymilkyTracker

> **Work in progress — not ready for use.**

A music tracker inspired by [MilkyTracker](https://milkytracker.org/) and ProTracker, built entirely in Rust. It targets both **web** (WebAssembly) and **native desktop** from a single codebase, bringing the classic module tracker experience to the browser and desktop with a pixel-art retro aesthetic — without requiring users to memorize arcane keyboard commands.

## Features

- **Pattern editor** — compose music in the classic tracker grid (XM format primary, MOD legacy support)
- **Built-in soundset** — [Freepats](http://freepats.zenvoid.org/) General MIDI library included out of the box
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

The technology stack has been finalized and implementation is about to begin. There is no working software yet. Contributions and feedback are welcome, but expect significant breaking changes at any stage.

See [`doc/TODOs.md`](doc/TODOs.md) for the phased task list and [`doc/product_design.md`](doc/product_design.md) for the full product vision.

## Naming Origin

The author is lactose intolerant, so he chose a name that suits him — paying homage to and parodying MilkyTracker.

## License

Copyright 2026 HUIHONG YOU

SoymilkyTracker is free software: you can redistribute it and/or modify it under the terms of the [GNU General Public License](LICENSE) as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
