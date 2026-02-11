use monitor_common::core::{AudioClip, HitSound};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext};

pub struct AudioEngine {
    ctx: AudioContext,
    music_buffer: Option<AudioBuffer>,
    music_source: Option<AudioBufferSourceNode>,
    hitsound_buffers: HashMap<HitSound, AudioBuffer>,
    start_time: f64, // context.currentTime when play started
    offset: f32,     // chart offset
}

impl AudioEngine {
    pub fn new() -> Result<Self, JsValue> {
        let ctx = AudioContext::new()?;
        Ok(Self {
            ctx,
            music_buffer: None,
            music_source: None,
            hitsound_buffers: HashMap::new(),
            start_time: 0.0,
            offset: 0.0,
        })
    }

    pub fn set_music(&mut self, clip: &AudioClip) -> Result<(), JsValue> {
        let buffer = self.ctx.create_buffer(
            clip.channel_count as u32,
            (clip.samples.len() / clip.channel_count as usize) as u32,
            clip.sample_rate as f32,
        )?;

        for channel in 0..clip.channel_count {
            let mut channel_data =
                Vec::with_capacity(clip.samples.len() / clip.channel_count as usize);
            for i in (channel as usize..clip.samples.len()).step_by(clip.channel_count as usize) {
                channel_data.push(clip.samples[i]);
            }
            buffer.copy_to_channel(&channel_data, channel as i32)?;
        }

        self.music_buffer = Some(buffer);
        Ok(())
    }

    pub fn set_hitsound(&mut self, kind: HitSound, clip: &AudioClip) -> Result<(), JsValue> {
        let buffer = self.ctx.create_buffer(
            clip.channel_count as u32,
            (clip.samples.len() / clip.channel_count as usize) as u32,
            clip.sample_rate as f32,
        )?;

        for channel in 0..clip.channel_count {
            let mut channel_data =
                Vec::with_capacity(clip.samples.len() / clip.channel_count as usize);
            for i in (channel as usize..clip.samples.len()).step_by(clip.channel_count as usize) {
                channel_data.push(clip.samples[i]);
            }
            buffer.copy_to_channel(&channel_data, channel as i32)?;
        }

        self.hitsound_buffers.insert(kind, buffer);
        Ok(())
    }

    pub fn play(&mut self, start_time: f32) -> Result<(), JsValue> {
        let current = self.ctx.current_time();
        // Audio starts at start_time + offset
        let audio_start_pos = start_time + self.offset;
        self.start_time = current - audio_start_pos as f64;

        if let Some(buffer) = &self.music_buffer {
            let source = self.ctx.create_buffer_source()?;
            source.set_buffer(Some(buffer));
            // Explicitly cast to BaseAudioContext to access destination()
            let base_ctx: &web_sys::BaseAudioContext = self.ctx.as_ref();
            source.connect_with_audio_node(&base_ctx.destination())?;

            if audio_start_pos >= 0.0 {
                source.start_with_when_and_grain_offset(current, audio_start_pos as f64)?;
            } else {
                // Future start
                source.start_with_when(current - audio_start_pos as f64)?;
            }

            self.music_source = Some(source);
        }
        let _ = self.ctx.resume()?;
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), JsValue> {
        if let Some(source) = self.music_source.take() {
            let _ = source.stop_with_when(0.0);
        }
        Ok(())
    }

    pub fn play_hitsound(&self, kind: &HitSound) -> Result<(), JsValue> {
        if let Some(buffer) = self.hitsound_buffers.get(kind) {
            let source = self.ctx.create_buffer_source()?;
            source.set_buffer(Some(buffer));
            let base_ctx: &web_sys::BaseAudioContext = self.ctx.as_ref();
            source.connect_with_audio_node(&base_ctx.destination())?;
            source.start()?;
        }
        Ok(())
    }

    pub fn get_time(&self) -> f32 {
        (self.ctx.current_time() - self.start_time) as f32 - self.offset
    }

    pub fn set_offset(&mut self, offset: f32) {
        self.offset = offset;
    }
}
