// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Native audio backend using `cpal`.
//!
//! Stereo output: the fill callback receives an interleaved `[L0, R0, L1, R1, ...]`
//! buffer. Mono or >2-channel devices are handled by clamping channel index to L/R.

use anyhow::Context as _;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream};

use super::{AudioBackend, FillCallback};

pub struct NativeAudioBackend {
    /// Holds the live cpal stream; dropping it stops playback.
    stream: Option<Stream>,
}

impl Default for NativeAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeAudioBackend {
    pub fn new() -> Self {
        Self { stream: None }
    }
}

impl AudioBackend for NativeAudioBackend {
    fn start(&mut self, fill: FillCallback) -> anyhow::Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("no default audio output device")?;
        let supported = device
            .default_output_config()
            .context("no default output config")?;
        let channels = supported.channels() as usize;
        let config: cpal::StreamConfig = supported.clone().into();

        let stream = match supported.sample_format() {
            SampleFormat::F32 => build_stream::<f32>(&device, &config, channels, fill),
            SampleFormat::I16 => build_stream::<i16>(&device, &config, channels, fill),
            SampleFormat::U16 => build_stream::<u16>(&device, &config, channels, fill),
            fmt => anyhow::bail!("unsupported sample format: {fmt:?}"),
        }?;
        stream.play().context("failed to start cpal stream")?;
        self.stream = Some(stream);
        Ok(())
    }

    fn stop(&mut self) {
        // Dropping the Stream pauses playback and releases the device.
        self.stream = None;
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    mut fill: FillCallback,
) -> anyhow::Result<Stream>
where
    T: SizedSample + FromSample<f32>,
{
    // Scratch buffer for stereo-interleaved f32 samples, reused each callback.
    let mut interleaved: Vec<f32> = Vec::new();

    let stream = device.build_output_stream(
        config,
        move |output: &mut [T], _info: &cpal::OutputCallbackInfo| {
            let frames = output.len() / channels;
            interleaved.resize(frames * 2, 0.0_f32);
            fill(&mut interleaved);

            for (frame_idx, frame) in output.chunks_mut(channels).enumerate() {
                let l = interleaved[frame_idx * 2];
                let r = interleaved[frame_idx * 2 + 1];
                for (ch, sample) in frame.iter_mut().enumerate() {
                    *sample = T::from_sample(if ch == 0 { l } else { r });
                }
            }
        },
        |err| tracing::error!("cpal stream error: {err}"),
        None,
    )?;
    Ok(stream)
}
