// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Proof-of-concept: Rust WASM orchestrating a Web Audio AudioWorklet.
//!
//! The audio *processing* (sine wave generation) runs inside an AudioWorklet
//! processor — a separate, high-priority audio thread provided by the browser.
//! Rust/WASM controls the setup: creating the AudioContext, bundling the
//! processor script as a Blob URL, loading it as an AudioWorklet module, and
//! connecting the node to the audio destination.
//!
//! In the full SoymilkyTracker app, the processor script will be replaced by a
//! WASM module that runs the tracker engine (fundsp DSP graph + oxisynth), and
//! audio parameters will be driven from the main WASM thread via MessagePort /
//! SharedArrayBuffer.

use js_sys::Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{AudioContext, AudioWorkletNode, Blob, BlobPropertyBag, HtmlButtonElement, Url};

/// AudioWorklet processor script embedded as a string literal.
///
/// Generates a continuous 440 Hz sine wave at 25% amplitude.
/// `sampleRate` is a global available inside AudioWorkletGlobalScope.
const PROCESSOR_JS: &str = r#"
class SineProcessor extends AudioWorkletProcessor {
    constructor() {
        super();
        this._phase = 0.0;
    }
    process(_inputs, outputs, _parameters) {
        const channel = outputs[0][0];
        if (!channel) return true;
        for (let i = 0; i < channel.length; i++) {
            channel[i] = Math.sin(this._phase) * 0.25;
            this._phase += (2.0 * Math.PI * 440.0) / sampleRate;
            if (this._phase >= 2.0 * Math.PI) {
                this._phase -= 2.0 * Math.PI;
            }
        }
        return true; // keep processor alive
    }
}
registerProcessor('sine-processor', SineProcessor);
"#;

/// Entry point called by wasm-bindgen after WASM initialisation.
/// Wires the Start button to the async audio initialisation routine.
#[wasm_bindgen(start)]
pub fn main() {
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");

    let btn = document
        .get_element_by_id("start-btn")
        .expect("no #start-btn element in HTML")
        .dyn_into::<HtmlButtonElement>()
        .unwrap();

    let on_click = Closure::<dyn FnMut()>::new(move || {
        spawn_local(async move {
            set_status("Starting audio…");
            match init_audio().await {
                Ok(()) => set_status("▶ Playing — 440 Hz sine wave via AudioWorklet"),
                Err(e) => set_status(&format!("Error: {:?}", e)),
            }
        });
    });

    btn.set_onclick(Some(on_click.as_ref().unchecked_ref()));
    on_click.forget(); // keep the closure alive for the lifetime of the page
}

fn set_status(msg: &str) {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("status"))
    {
        el.set_text_content(Some(msg));
    }
}

/// Creates an AudioContext, loads the sine processor as an AudioWorklet module,
/// and connects it to the audio output.
async fn init_audio() -> Result<(), JsValue> {
    let ctx = AudioContext::new()?;

    // Bundle the processor JS as a Blob URL so AudioWorklet.addModule() can
    // load it without needing a separate served file.
    let parts = Array::new();
    parts.push(&JsValue::from_str(PROCESSOR_JS));
    let blob_opts = BlobPropertyBag::new();
    blob_opts.set_type("application/javascript");
    let blob = Blob::new_with_str_sequence_and_options(&parts, &blob_opts)?;
    let url = Url::create_object_url_with_blob(&blob)?;

    // AudioWorklet.addModule() returns a Promise — await it.
    JsFuture::from(ctx.audio_worklet()?.add_module(&url)?).await?;
    Url::revoke_object_url(&url)?; // Blob URL no longer needed after module load

    // Create the AudioWorkletNode and connect it to the speakers.
    let node = AudioWorkletNode::new(&ctx, "sine-processor")?;
    node.connect_with_audio_node(&ctx.destination())?;

    Ok(())
}
