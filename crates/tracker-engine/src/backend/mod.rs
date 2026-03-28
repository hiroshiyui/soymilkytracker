// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Audio I/O abstraction. The audio engine logic is shared; only the I/O
//! layer differs per compile target.
//!
//! - Native:  `NativeAudioBackend` backed by `cpal`
//! - WASM:    `WasmAudioBackend` backed by Web Audio `AudioWorklet`

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeAudioBackend;
#[cfg(target_arch = "wasm32")]
pub use wasm::WasmAudioBackend;

/// Platform-agnostic interface for audio output.
pub trait AudioBackend: Send {
    /// Start audio output, driven by the provided sample callback.
    fn start(&mut self) -> anyhow::Result<()>;
    /// Stop audio output and release resources.
    fn stop(&mut self);
}
