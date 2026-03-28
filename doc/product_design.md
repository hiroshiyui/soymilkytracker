# Product Design

I want to create a web-based application inspired by MilkyTracker. It should be built using WebAssembly and have the core functionalities of MilkyTracker, but with a more user-friendly UI/UX. The goal is to eliminate the need for users to memorize the control commands typically found in module trackers.

## Technology Stack (Finalized 2026-03-28)

### Client (dual-target: Web WASM + Native Desktop)
| Layer | Choice |
|---|---|
| UI framework | `egui` + `eframe` (native + WASM web, out of the box) |
| SF2 synthesis | `oxisynth` (pure Rust, all targets) |
| DSP / audio graph | `fundsp` (pure Rust, all targets) |
| Module format | XM (primary), MOD (legacy) |
| Audio I/O — Web | Web Audio `AudioWorklet` via `wasm-bindgen` |
| Audio I/O — Native | `cpal` (cross-platform: Windows, macOS, Linux) |

The audio engine logic is shared across targets behind an `AudioBackend` trait, gated with `#[cfg(target_arch = "wasm32")]`.

### Backend
| Layer | Choice |
|---|---|
| Framework | Rust + `axum` |
| Database | PostgreSQL |
| File storage | Local filesystem (self-hosted, single server instance) |
| Auth | JWT / OAuth (GitHub, Google) |

## Features

- Built-in Freepats instrument sound library.
- Allow users to upload and load custom sound font files.
- User compositions can be saved to a remote server.
- User compositions can be downloaded.
- A clean and elegant user profile system.
- Creators can publish their own works and playlists.
- Playback of works and playlists via the WebAssembly-based player.

## UI/UX Design

- The overall interface should emulate the style of MilkyTracker and Protracker.
- Use pixel art style graphics and fonts as much as possible.
