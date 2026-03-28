// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Proof-of-concept: cpal producing a 440 Hz sine wave on the native desktop.
//!
//! Validates the NativeAudioBackend path of the AudioBackend trait.
//! The data callback pattern here maps directly to what NativeAudioBackend
//! will use to feed samples from the tracker engine into cpal.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use std::f32::consts::TAU;

fn main() {
    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .expect("no output device available");
    println!("Output device : {}", device.name().unwrap_or_default());

    let config = device
        .default_output_config()
        .expect("failed to get default output config");
    println!(
        "Sample format : {:?}  |  sample rate: {} Hz  |  channels: {}",
        config.sample_format(),
        config.sample_rate().0,
        config.channels(),
    );

    match config.sample_format() {
        SampleFormat::F32 => play::<f32>(&device, &config.into()),
        SampleFormat::I16 => play::<i16>(&device, &config.into()),
        SampleFormat::U16 => play::<u16>(&device, &config.into()),
        fmt => panic!("unsupported sample format: {fmt:?}"),
    }
}

fn play<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
) {
    let sample_rate = config.sample_rate.0 as f32;
    let channels   = config.channels as usize;
    let freq       = 440.0_f32; // A4
    let mut phase  = 0.0_f32;

    let stream = device
        .build_output_stream(
            config,
            move |output: &mut [T], _| {
                for frame in output.chunks_mut(channels) {
                    // Generate one sine sample and write it to every channel.
                    let sample = T::from_sample((phase * TAU).sin() * 0.3_f32);
                    phase = (phase + freq / sample_rate) % 1.0;
                    for ch in frame.iter_mut() {
                        *ch = sample;
                    }
                }
            },
            |err| eprintln!("stream error: {err}"),
            None, // no timeout
        )
        .expect("failed to build output stream");

    stream.play().expect("failed to start stream");

    println!("Playing 440 Hz sine wave via cpal. Press Enter to stop.");
    std::io::stdin().read_line(&mut String::new()).ok();
}
