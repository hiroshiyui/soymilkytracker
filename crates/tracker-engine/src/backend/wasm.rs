// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! WASM audio backend using Web Audio API `AudioWorklet`.
//!
//! ## Architecture
//!
//! ```text
//! Rust (main WASM thread)          │  AudioWorklet thread
//! ─────────────────────────────────┼──────────────────────────────
//! tick_audio() via requestAnimationFrame
//!   └─ fill callback → interleaved f32 chunk
//!   └─ post Float32Array via MessagePort  →  TrackerProcessor
//!                                           └─ dequeue & write to outputs
//! ```
//!
//! Audio chunks are pre-buffered `LOOKAHEAD_CHUNKS` frames ahead of the
//! AudioContext clock to avoid underruns.  On each animation frame the
//! scheduler tops up the queue as needed.
//!
//! `start()` returns immediately; the async `AudioWorklet` registration
//! happens in a `spawn_local` task.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js_sys::Float32Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{AudioContext, AudioWorkletNode, AudioWorkletNodeOptions};

use super::{AudioBackend, FillCallback};

// ---------------------------------------------------------------------------
// AudioWorklet processor script (loaded via Blob URL)
// ---------------------------------------------------------------------------

const PROCESSOR_JS: &str = r#"
class TrackerProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this._queue = [];
    this._pos   = 0;
    this.port.onmessage = (e) => {
      if (e.data && e.data.samples) this._queue.push(e.data.samples);
    };
  }
  process(inputs, outputs) {
    const out = outputs[0];
    const L   = out[0];
    const R   = out.length > 1 ? out[1] : out[0];
    const n   = L.length;
    let w = 0;
    while (w < n && this._queue.length > 0) {
      const chunk  = this._queue[0];
      const frames = chunk.length >> 1;     // stereo-interleaved → frame count
      const avail  = frames - this._pos;
      const take   = Math.min(avail, n - w);
      for (let i = 0; i < take; i++) {
        L[w + i] = chunk[(this._pos + i) << 1];
        R[w + i] = chunk[((this._pos + i) << 1) + 1];
      }
      this._pos += take;
      w         += take;
      if (this._pos >= frames) { this._queue.shift(); this._pos = 0; }
    }
    // Silence any unwritten samples (buffer underrun guard).
    for (let i = w; i < n; i++) { L[i] = 0; if (R !== L) R[i] = 0; }
    return true;
  }
}
registerProcessor('tracker-processor', TrackerProcessor);
"#;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Stereo frames per chunk posted to the worklet (~46 ms at 44100 Hz).
const CHUNK_FRAMES: usize = 2048;

/// How many chunks to keep ahead of the AudioContext playback clock.
const LOOKAHEAD_CHUNKS: usize = 4;

// ---------------------------------------------------------------------------
// Internal audio state (lives inside the RAF closure)
// ---------------------------------------------------------------------------

struct AudioState {
    ctx: AudioContext,
    node: AudioWorkletNode,
    fill: FillCallback,
    sent_frames: u64,
    start_time: f64,
    sample_rate: f64,
}

// ---------------------------------------------------------------------------
// Public backend struct
// ---------------------------------------------------------------------------

pub struct WasmAudioBackend {
    /// Set to false by `stop()` to halt the RAF loop.
    running: Rc<Cell<bool>>,
    /// Lets `stop()` close the AudioContext without holding the whole state.
    ctx_holder: Rc<RefCell<Option<AudioContext>>>,
    /// Keeps the RAF `Closure` object alive (dropping it breaks the JS reference).
    _raf: Rc<RefCell<Option<Closure<dyn FnMut()>>>>,
}

impl Default for WasmAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmAudioBackend {
    pub fn new() -> Self {
        Self {
            running: Rc::new(Cell::new(false)),
            ctx_holder: Rc::new(RefCell::new(None)),
            _raf: Rc::new(RefCell::new(None)),
        }
    }
}

impl AudioBackend for WasmAudioBackend {
    fn start(&mut self, fill: FillCallback) -> anyhow::Result<()> {
        self.running.set(true);
        let running = self.running.clone();
        let ctx_holder = self.ctx_holder.clone();
        let raf_holder = self._raf.clone();

        spawn_local(async move {
            if let Err(e) = setup_audio(fill, running.clone(), ctx_holder, raf_holder).await {
                tracing::error!("WasmAudioBackend setup failed: {e:?}");
                running.set(false);
            }
        });
        Ok(())
    }

    fn stop(&mut self) {
        self.running.set(false);
        // Drop the closure — the next RAF invocation won't reschedule.
        *self._raf.borrow_mut() = None;
        // Close the AudioContext asynchronously.
        let ctx_holder = self.ctx_holder.clone();
        spawn_local(async move {
            let ctx = ctx_holder.borrow().clone();
            if let Some(ctx) = ctx {
                if let Ok(promise) = ctx.close() {
                    let _ = JsFuture::from(promise).await;
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Async setup
// ---------------------------------------------------------------------------

async fn setup_audio(
    fill: FillCallback,
    running: Rc<Cell<bool>>,
    ctx_holder: Rc<RefCell<Option<AudioContext>>>,
    raf_holder: Rc<RefCell<Option<Closure<dyn FnMut()>>>>,
) -> anyhow::Result<()> {
    // 1. Create AudioContext.
    let ctx = AudioContext::new().map_err(|e| anyhow::anyhow!("AudioContext::new: {e:?}"))?;
    let sample_rate = ctx.sample_rate() as f64;
    *ctx_holder.borrow_mut() = Some(ctx.clone());

    // 2. Register the worklet processor via a Blob URL.
    let url = blob_url(PROCESSOR_JS)?;
    let worklet = ctx
        .audio_worklet()
        .map_err(|e| anyhow::anyhow!("audio_worklet(): {e:?}"))?;
    JsFuture::from(
        worklet
            .add_module(&url)
            .map_err(|e| anyhow::anyhow!("add_module: {e:?}"))?,
    )
    .await
    .map_err(|e| anyhow::anyhow!("add_module await: {e:?}"))?;
    web_sys::Url::revoke_object_url(&url).ok(); // free blob memory

    // 3. Create the AudioWorkletNode with stereo output.
    let opts = AudioWorkletNodeOptions::new();
    let ch_counts = js_sys::Array::of1(&JsValue::from(2u32));
    js_sys::Reflect::set(
        opts.as_ref(),
        &JsValue::from_str("outputChannelCount"),
        &ch_counts,
    )
    .map_err(|e| anyhow::anyhow!("Reflect::set outputChannelCount: {e:?}"))?;
    let node = AudioWorkletNode::new_with_options(&ctx, "tracker-processor", &opts)
        .map_err(|e| anyhow::anyhow!("AudioWorkletNode::new: {e:?}"))?;

    // 4. Connect node → speakers.
    node.connect_with_audio_node(&ctx.destination())
        .map_err(|e| anyhow::anyhow!("connect: {e:?}"))?;

    // 5. Hand off to the RAF loop.
    let start_time = ctx.current_time();
    let state = Rc::new(RefCell::new(AudioState {
        ctx,
        node,
        fill,
        sent_frames: 0,
        start_time,
        sample_rate,
    }));
    start_raf_loop(state, running, raf_holder);
    Ok(())
}

// ---------------------------------------------------------------------------
// requestAnimationFrame loop
// ---------------------------------------------------------------------------

fn start_raf_loop(
    state: Rc<RefCell<AudioState>>,
    running: Rc<Cell<bool>>,
    raf_holder: Rc<RefCell<Option<Closure<dyn FnMut()>>>>,
) {
    // `holder2` is captured by the closure so it can re-register itself.
    let holder2 = raf_holder.clone();

    let closure = Closure::wrap(Box::new(move || {
        if !running.get() {
            return; // stop re-scheduling; cycle broken when `_raf` is set to None
        }
        tick_audio(&state);
        if let Some(window) = web_sys::window() {
            if let Some(cb) = holder2.borrow().as_ref() {
                window
                    .request_animation_frame(cb.as_ref().unchecked_ref())
                    .ok();
            }
        }
    }) as Box<dyn FnMut()>);

    // Schedule the first frame before storing — the store completes before the
    // RAF fires because everything runs on the single JS event loop.
    if let Some(window) = web_sys::window() {
        window
            .request_animation_frame(closure.as_ref().unchecked_ref())
            .ok();
    }
    *raf_holder.borrow_mut() = Some(closure);
}

// ---------------------------------------------------------------------------
// Per-frame audio filling
// ---------------------------------------------------------------------------

fn tick_audio(state: &Rc<RefCell<AudioState>>) {
    // Compute how many frames the worklet has consumed by now.
    let (target_frames, sent_frames) = {
        let s = state.borrow();
        let elapsed = s.ctx.current_time() - s.start_time;
        let consumed = (elapsed * s.sample_rate) as u64;
        let target = consumed + (CHUNK_FRAMES * LOOKAHEAD_CHUNKS) as u64;
        (target, s.sent_frames)
    };

    if sent_frames >= target_frames {
        return;
    }

    let chunks = (target_frames - sent_frames).div_ceil(CHUNK_FRAMES as u64) as usize;
    let mut buf = vec![0.0_f32; CHUNK_FRAMES * 2];

    for _ in 0..chunks {
        // Call the fill callback (mutable borrow only for the duration of the call).
        {
            let mut s = state.borrow_mut();
            let fill = &mut s.fill;
            fill(&mut buf);
            s.sent_frames += CHUNK_FRAMES as u64;
        }
        // Post the rendered chunk to the worklet.
        {
            let s = state.borrow();
            send_chunk(&s.node, &buf);
        }
    }
}

fn send_chunk(node: &AudioWorkletNode, interleaved: &[f32]) {
    let samples = Float32Array::new_with_length(interleaved.len() as u32);
    samples.copy_from(interleaved);
    let msg = js_sys::Object::new();
    js_sys::Reflect::set(&msg, &JsValue::from_str("samples"), &samples).ok();
    if let Ok(port) = node.port() {
        port.post_message(&msg).ok();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn blob_url(js_src: &str) -> anyhow::Result<String> {
    let bytes = js_src.as_bytes();
    let uint8 = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    uint8.copy_from(bytes);
    let array = js_sys::Array::of1(&uint8);
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("application/javascript");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&array, &opts)
        .map_err(|e| anyhow::anyhow!("Blob::new: {e:?}"))?;
    web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| anyhow::anyhow!("createObjectURL: {e:?}"))
}
