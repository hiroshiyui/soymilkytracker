// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Proof-of-concept: oxisynth (SF2 synthesis) running in WASM, playing a note
//! via Web Audio AudioBufferSourceNode.
//!
//! Flow:
//!   1. Fetch the SF2 file from the server (served by `trunk serve`).
//!   2. Pass the bytes to oxisynth to load the soundfont.
//!   3. Send ProgramChange + NoteOn, render N samples one at a time, NoteOff.
//!   4. Copy the rendered PCM into a Web Audio AudioBuffer and play it once.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{AudioBufferSourceNode, AudioContext, HtmlButtonElement, Response};

const SAMPLE_RATE: f32 = 44_100.0;
const NUM_SAMPLES: usize = (SAMPLE_RATE as usize) * 2; // 2 seconds
const RELEASE_AT:  usize = (SAMPLE_RATE as usize) * 3 / 2; // note off at 1.5 s
const CHANNEL:     u8  = 0;
const PROGRAM:     u8  = 0;   // GM bank 0, program 0 = Acoustic Grand Piano
const NOTE_A4:     u8  = 69;
const VELOCITY:    u8  = 100;

#[wasm_bindgen(start)]
pub fn main() {
    let document = web_sys::window().unwrap().document().unwrap();
    let btn = document
        .get_element_by_id("play-btn")
        .unwrap()
        .dyn_into::<HtmlButtonElement>()
        .unwrap();

    let on_click = Closure::<dyn FnMut()>::new(move || {
        spawn_local(async move {
            set_status("Fetching SF2…");
            match run().await {
                Ok(())  => set_status("▶ Playing A4 (Acoustic Grand Piano) via oxisynth"),
                Err(e)  => set_status(&format!("Error: {:?}", e)),
            }
        });
    });

    btn.set_onclick(Some(on_click.as_ref().unchecked_ref()));
    on_click.forget();
}

fn set_status(msg: &str) {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("status"))
    {
        el.set_text_content(Some(msg));
    }
}

async fn run() -> Result<(), JsValue> {
    // ── 1. Fetch the SF2 ────────────────────────────────────────────────────
    let resp: Response = JsFuture::from(
        web_sys::window().unwrap().fetch_with_str("TimGM6mb.sf2"),
    ).await?.dyn_into()?;

    let array_buffer = JsFuture::from(resp.array_buffer()?).await?;
    let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();

    set_status("SF2 loaded, rendering samples…");

    // ── 2. Initialise oxisynth ───────────────────────────────────────────────
    let desc = oxisynth::SynthDescriptor {
        sample_rate: SAMPLE_RATE,
        ..Default::default()
    };
    let mut synth = oxisynth::Synth::new(desc)
        .map_err(|e| JsValue::from_str(&format!("Synth init error: {e:?}")))?;

    let font = oxisynth::SoundFont::load(&mut std::io::Cursor::new(bytes))
        .map_err(|e| JsValue::from_str(&format!("SF2 load error: {e}")))?;
    synth.add_font(font, true);

    // Select GM program (Acoustic Grand Piano)
    synth.send_event(oxisynth::MidiEvent::ProgramChange {
        channel:    CHANNEL,
        program_id: PROGRAM,
    }).map_err(|e| JsValue::from_str(&format!("ProgramChange error: {e:?}")))?;

    // ── 3. Render samples ───────────────────────────────────────────────────
    let mut left  = Vec::with_capacity(NUM_SAMPLES);
    let mut right = Vec::with_capacity(NUM_SAMPLES);

    synth.send_event(oxisynth::MidiEvent::NoteOn {
        channel: CHANNEL,
        key:     NOTE_A4,
        vel:     VELOCITY,
    }).map_err(|e| JsValue::from_str(&format!("NoteOn error: {e:?}")))?;

    for i in 0..NUM_SAMPLES {
        if i == RELEASE_AT {
            synth.send_event(oxisynth::MidiEvent::NoteOff {
                channel: CHANNEL,
                key:     NOTE_A4,
            }).ok();
        }
        let (l, r) = synth.read_next();
        left.push(l);
        right.push(r);
    }

    set_status("PCM ready, handing to Web Audio…");

    // ── 4. Play via Web Audio AudioBuffer ───────────────────────────────────
    let ctx = AudioContext::new()?;
    let audio_buf = ctx.create_buffer(2, NUM_SAMPLES as u32, SAMPLE_RATE)?;
    audio_buf.copy_to_channel(&left,  0)?;
    audio_buf.copy_to_channel(&right, 1)?;

    let source: AudioBufferSourceNode = ctx.create_buffer_source()?;
    source.set_buffer(Some(&audio_buf));
    source.connect_with_audio_node(&ctx.destination())?;
    source.start()?;

    Ok(())
}
