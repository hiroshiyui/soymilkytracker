// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! WASM audio backend using Web Audio API `AudioWorklet`.

use super::AudioBackend;

pub struct WasmAudioBackend {
    // web_sys::AudioContext will be held here.
    // Populated in Phase 1.
}

impl WasmAudioBackend {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioBackend for WasmAudioBackend {
    fn start(&mut self) -> anyhow::Result<()> {
        // TODO (Phase 1): create AudioContext, register AudioWorklet module,
        //                 create AudioWorkletNode and connect to destination.
        Ok(())
    }

    fn stop(&mut self) {
        // TODO (Phase 1): close AudioContext.
    }
}
