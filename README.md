# SoymilkyTracker

A web-based music tracker inspired by [MilkyTracker](https://milkytracker.org/) and ProTracker, built entirely in Rust and compiled to WebAssembly. It aims to bring the classic module tracker experience to the browser with a pixel-art retro aesthetic — without requiring users to memorize arcane keyboard commands.

## Features

- Pattern-based composition editor (XM format primary, MOD legacy support)
- Built-in [Freepats](http://freepats.zenvoid.org/) General MIDI sound library
- Upload and use custom SF2 / WAV soundfonts
- Save and download compositions
- Publish works and playlists; embedded WASM player for sharing
- User profiles and community discovery

## Technology Stack

### Client (WebAssembly)

| Layer | Library |
|---|---|
| UI framework | [`egui`](https://github.com/emilk/egui) + [`eframe`](https://github.com/emilk/egui) |
| Audio engine | Rust → WASM via Web Audio `AudioWorklet` |
| DSP / audio graph | [`fundsp`](https://github.com/SamiPerttu/fundsp) |
| SF2 synthesis | [`oxisynth`](https://github.com/PolyMeilex/OxiSynth) |
| WASM bindings | [`wasm-bindgen`](https://github.com/rustwasm/wasm-bindgen) |

### Backend

| Layer | Choice |
|---|---|
| Framework | Rust + [`axum`](https://github.com/tokio-rs/axum) |
| Database | PostgreSQL |
| File storage | S3-compatible object storage |
| Auth | JWT / OAuth (GitHub, Google) |

## Project Status

> **Work in progress — not ready for use.**

The technology stack has been finalized and implementation is about to begin. There is no working software yet. See [`doc/TODOs.md`](doc/TODOs.md) for the phased task list.

## License

Copyright 2026 HUIHONG YOU

This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

See [`LICENSE`](LICENSE) for the full text.
