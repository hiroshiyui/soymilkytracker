// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! High-level audio controller: wires [`Player`] to an [`AudioBackend`].
//!
//! [`TrackerAudio`] owns both the backend and the player and exposes simple
//! transport controls (`play`, `pause`, `stop`, `seek`, `load`) without
//! exposing threading details to callers.
//!
//! ## Threading model
//!
//! | Target | Player handle | Reason |
//! |--------|--------------|--------|
//! | Native | `Arc<Mutex<Player>>` | cpal calls the fill callback on a dedicated audio thread |
//! | WASM   | `Rc<RefCell<Player>>` | WASM is single-threaded; `Rc` avoids the `Send` requirement |
//!
//! ## Example (native)
//! ```ignore
//! let backend = Box::new(NativeAudioBackend::new());
//! let mut audio = TrackerAudio::new(backend);
//! audio.load(Arc::new(xm::parse(&bytes)?));
//! audio.play()?;
//! // later…
//! audio.pause();
//! audio.seek(2, 0);
//! audio.play()?;
//! audio.stop();
//! ```

use std::sync::Arc;

use crate::backend::AudioBackend;
use crate::player::{PlaybackPosition, Player};
use crate::xm::XmModule;

// ── Platform-specific player handle ──────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

// ── TrackerAudio ──────────────────────────────────────────────────────────────

/// High-level controller: owns the audio backend and the XM player.
pub struct TrackerAudio {
    backend: Box<dyn AudioBackend>,

    #[cfg(not(target_arch = "wasm32"))]
    player: Option<Arc<Mutex<Player>>>,

    #[cfg(target_arch = "wasm32")]
    player: Option<Rc<RefCell<Player>>>,

    /// `true` once `backend.start()` has been called (and not yet followed by `stop()`).
    stream_running: bool,
}

impl TrackerAudio {
    pub fn new(backend: Box<dyn AudioBackend>) -> Self {
        Self {
            backend,
            player: None,
            stream_running: false,
        }
    }

    /// Load (or replace) the module.
    ///
    /// Any current playback is stopped first.  The player is initialised at
    /// the sample rate reported by the backend.
    pub fn load(&mut self, module: Arc<XmModule>) {
        self.stop();
        let sr = self.backend.preferred_sample_rate();
        let player = Player::new(module, sr);
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.player = Some(Arc::new(Mutex::new(player)));
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.player = Some(Rc::new(RefCell::new(player)));
        }
    }

    /// Start or resume playback.
    ///
    /// Opens the audio stream on the first call; after a [`pause`][Self::pause]
    /// it simply resumes the player without re-opening the stream.
    ///
    /// Does nothing if no module has been loaded.
    pub fn play(&mut self) -> anyhow::Result<()> {
        let Some(player) = self.player.as_ref() else {
            return Ok(());
        };

        // Tell the player to start advancing.
        #[cfg(not(target_arch = "wasm32"))]
        player.lock().unwrap().play();
        #[cfg(target_arch = "wasm32")]
        player.borrow_mut().play();

        // Open the stream once; on subsequent play() calls the stream is
        // already running so we only needed to flip player.playing above.
        if !self.stream_running {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let player_cb = Arc::clone(player);
                self.backend.start(Box::new(move |buf| {
                    if let Ok(mut p) = player_cb.lock() {
                        p.fill(buf);
                    }
                }))?;
            }
            #[cfg(target_arch = "wasm32")]
            {
                let player_cb = Rc::clone(player);
                self.backend.start(Box::new(move |buf| {
                    player_cb.borrow_mut().fill(buf);
                }))?;
            }
            self.stream_running = true;
        }
        Ok(())
    }

    /// Pause playback.
    ///
    /// The audio stream stays open so resuming via [`play`][Self::play] has no
    /// device round-trip overhead.  The player outputs silence while paused.
    pub fn pause(&mut self) {
        if let Some(player) = self.player.as_ref() {
            #[cfg(not(target_arch = "wasm32"))]
            player.lock().unwrap().pause();
            #[cfg(target_arch = "wasm32")]
            player.borrow_mut().pause();
        }
    }

    /// Stop playback, rewind to the beginning, and close the audio stream.
    pub fn stop(&mut self) {
        if let Some(player) = self.player.as_ref() {
            #[cfg(not(target_arch = "wasm32"))]
            player.lock().unwrap().stop();
            #[cfg(target_arch = "wasm32")]
            player.borrow_mut().stop();
        }
        if self.stream_running {
            self.backend.stop();
            self.stream_running = false;
        }
    }

    /// Seek to an absolute position in the order list.
    ///
    /// Silences all channels at the new position; playback continues if it
    /// was already running.
    pub fn seek(&mut self, order: usize, row: usize) {
        if let Some(player) = self.player.as_ref() {
            #[cfg(not(target_arch = "wasm32"))]
            player.lock().unwrap().set_position(order, row);
            #[cfg(target_arch = "wasm32")]
            player.borrow_mut().set_position(order, row);
        }
    }

    /// Snapshot of the current playback position.
    ///
    /// Returns `None` if no module has been loaded.
    pub fn position(&self) -> Option<PlaybackPosition> {
        self.player.as_ref().map(|p| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                p.lock().unwrap().position()
            }
            #[cfg(target_arch = "wasm32")]
            {
                p.borrow().position()
            }
        })
    }

    /// Returns `true` while the player is advancing.
    pub fn is_playing(&self) -> bool {
        self.player.as_ref().is_some_and(|p| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                p.lock().unwrap().is_playing()
            }
            #[cfg(target_arch = "wasm32")]
            {
                p.borrow().is_playing()
            }
        })
    }
}
