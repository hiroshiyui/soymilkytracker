// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Native audio backend using `cpal`.

use super::AudioBackend;

pub struct NativeAudioBackend {
    // cpal stream will be held here to keep it alive.
    // Populated in Phase 1.
}

impl NativeAudioBackend {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioBackend for NativeAudioBackend {
    fn start(&mut self) -> anyhow::Result<()> {
        // TODO (Phase 1): initialise cpal host/device/stream.
        Ok(())
    }

    fn stop(&mut self) {
        // TODO (Phase 1): drop cpal stream.
    }
}
