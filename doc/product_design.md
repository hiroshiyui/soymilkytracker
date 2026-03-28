# Product Design

I want to create a web-based application inspired by MilkyTracker. It should be built using WebAssembly and have the core functionalities of MilkyTracker, but with a more user-friendly UI/UX. The goal is to eliminate the need for users to memorize the control commands typically found in module trackers.

## Technology Stack (Finalized 2026-03-28)

### Client
| Layer | Choice |
|---|---|
| Audio engine | Rust → WASM in Web Audio `AudioWorklet` |
| UI framework | `egui` + `eframe` (immediate-mode, WASM-ready) |
| SF2 synthesis | `oxisynth` (pure Rust, WASM-compatible) |
| DSP / audio graph | `fundsp` |
| WASM bindings | `wasm-bindgen` |
| Module format | XM (primary), MOD (legacy) |

### Backend
| Layer | Choice |
|---|---|
| Framework | Rust + `axum` |
| Database | PostgreSQL |
| File storage | S3-compatible object storage |
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
