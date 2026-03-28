# SoymilkyTracker

> **Work in progress — not ready for use.**

A web-based music tracker inspired by [MilkyTracker](https://milkytracker.org/) and ProTracker, built entirely in Rust and compiled to WebAssembly. It brings the classic module tracker experience to the browser with a pixel-art retro aesthetic — without requiring users to memorize arcane keyboard commands.

## Features

- **Pattern editor** — compose music in the classic tracker grid (XM format primary, MOD legacy support)
- **Built-in soundset** — [Freepats](http://freepats.zenvoid.org/) General MIDI library included out of the box
- **Custom instruments** — upload and use your own SF2 / WAV soundfonts
- **Cloud save** — save and download compositions from a remote server
- **Publishing** — publish works and playlists; share via an embedded WASM player
- **User profiles** — personal pages, community browsing, and discovery

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

The technology stack has been finalized and implementation is about to begin. There is no working software yet. Contributions and feedback are welcome, but expect significant breaking changes at any stage.

See [`doc/TODOs.md`](doc/TODOs.md) for the phased task list and [`doc/product_design.md`](doc/product_design.md) for the full product vision.

## Naming Origin

The author is lactose intolerant, so he chose a name that suits him — paying homage to and parodying MilkyTracker.

## License

Copyright 2026 HUIHONG YOU

SoymilkyTracker is free software: you can redistribute it and/or modify it under the terms of the [GNU General Public License](LICENSE) as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
