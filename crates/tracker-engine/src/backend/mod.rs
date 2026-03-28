// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Audio I/O abstraction. The audio engine logic is shared; only the I/O
//! layer differs per compile target.
//!
//! - Native:  [`NativeAudioBackend`] backed by `cpal`
//! - WASM:    [`WasmAudioBackend`] backed by Web Audio `AudioWorklet`

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeAudioBackend;
#[cfg(target_arch = "wasm32")]
pub use wasm::WasmAudioBackend;

/// Stereo-interleaved fill callback: `[L0, R0, L1, R1, ...]`.
///
/// The backend calls this whenever it needs a new chunk of samples.
/// On native it must be `Send + 'static` (audio runs on a separate thread);
/// on WASM it only needs `'static` (single-threaded).
#[cfg(not(target_arch = "wasm32"))]
pub type FillCallback = Box<dyn FnMut(&mut [f32]) + Send + 'static>;
#[cfg(target_arch = "wasm32")]
pub type FillCallback = Box<dyn FnMut(&mut [f32]) + 'static>;

/// Platform-agnostic interface for audio output.
pub trait AudioBackend {
    /// Start audio output, driving it with the provided sample callback.
    ///
    /// On WASM the `AudioWorklet` registration is async; `start` returns
    /// immediately and audio begins once the worklet is ready.
    fn start(&mut self, fill: FillCallback) -> anyhow::Result<()>;

    /// Stop audio output and release audio resources.
    fn stop(&mut self);
}
