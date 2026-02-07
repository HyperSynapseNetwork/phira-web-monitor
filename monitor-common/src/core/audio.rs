use serde::{Deserialize, Serialize};
use std::{error::Error, fs::File, path::Path};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream,
    meta::MetadataOptions, probe::Hint,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioClip {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channel_count: u16,
}

impl AudioClip {
    pub fn new(samples: Vec<f32>, sample_rate: u32, channel_count: u16) -> Self {
        Self {
            samples,
            sample_rate,
            channel_count,
        }
    }

    /// Load audio from path
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let src = File::open(path.as_ref())?;
        let mss = MediaSourceStream::new(Box::new(src), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.as_ref().extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;

        let mut format = probed.format;
        let track = format.default_track().ok_or("No default track found")?;

        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())?;

        let track_id = track.id;
        let mut sample_rate = 0;
        let mut channel_count = 0;
        let mut all_samples: Vec<f32> = Vec::new();

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(_)) => break,
                Err(e) => return Err(Box::new(e)),
            };
            if packet.track_id() != track_id {
                continue;
            }
            match decoder.decode(&packet) {
                Ok(decoded) => {
                    if sample_rate == 0 {
                        let spec = *decoded.spec();
                        sample_rate = spec.rate;
                        channel_count = spec.channels.count() as u16;
                    }

                    let mut sample_buf =
                        SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());

                    sample_buf.copy_interleaved_ref(decoded);
                    all_samples.extend_from_slice(sample_buf.samples());
                }
                Err(symphonia::core::errors::Error::IoError(_)) => break,
                Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
                Err(e) => return Err(Box::new(e)),
            }
        }

        if all_samples.is_empty() {
            Err("No audio data decoded".into())
        } else {
            Ok(Self::new(all_samples, sample_rate, channel_count))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{BufWriter, Write};
    use std::path::PathBuf;

    /// 辅助函数：生成一个简单的单声道 16-bit PCM WAV 文件
    /// 参数：文件路径、采样率、持续时间（秒）
    fn create_dummy_wav(path: &std::path::Path, sample_rate: u32, duration_secs: u32) {
        let spec_channels = 1u16;
        let spec_bits_per_sample = 16u16;
        let total_samples = sample_rate * duration_secs;
        let data_size = total_samples * (spec_channels as u32) * (spec_bits_per_sample as u32 / 8);
        let chunk_size = 36 + data_size;

        let f = File::create(path).expect("Failed to create temp wav file");
        let mut writer = BufWriter::new(f);

        // --- 写入 WAV 文件头 (Little Endian) ---
        // 1. RIFF Chunk
        writer.write_all(b"RIFF").unwrap();
        writer.write_all(&chunk_size.to_le_bytes()).unwrap();
        writer.write_all(b"WAVE").unwrap();

        // 2. fmt Chunk
        writer.write_all(b"fmt ").unwrap();
        writer.write_all(&16u32.to_le_bytes()).unwrap(); // Subchunk1Size (16 for PCM)
        writer.write_all(&1u16.to_le_bytes()).unwrap(); // AudioFormat (1 for PCM)
        writer.write_all(&spec_channels.to_le_bytes()).unwrap(); // NumChannels
        writer.write_all(&sample_rate.to_le_bytes()).unwrap(); // SampleRate

        // ByteRate = SampleRate * NumChannels * BitsPerSample/8
        let byte_rate = sample_rate * (spec_channels as u32) * (spec_bits_per_sample as u32 / 8);
        writer.write_all(&byte_rate.to_le_bytes()).unwrap();

        // BlockAlign = NumChannels * BitsPerSample/8
        let block_align = spec_channels * (spec_bits_per_sample / 8);
        writer.write_all(&block_align.to_le_bytes()).unwrap();
        writer
            .write_all(&spec_bits_per_sample.to_le_bytes())
            .unwrap();

        // 3. data Chunk
        writer.write_all(b"data").unwrap();
        writer.write_all(&data_size.to_le_bytes()).unwrap();

        // 4. 写入音频数据 (生成简单的静音数据或规律波形)
        // 我们写入一些递增的数据，方便验证，但因为是 i16，转换回 f32 应该是 [0, 1] 之间的值
        for i in 0..total_samples {
            // 模拟一个简单的波形数据
            let value = (i % 1000) as i16;
            writer.write_all(&value.to_le_bytes()).unwrap();
        }

        writer.flush().unwrap();
    }

    #[test]
    fn test_load_audio_clip_from_wav() {
        // 1. 准备测试文件路径
        let test_file_name = "temp_test_audio.wav";
        let path = PathBuf::from(test_file_name);

        // 2. 生成一个 44100Hz, 1秒长的测试 WAV 文件
        let expected_sample_rate = 44100;
        let expected_duration = 1;
        create_dummy_wav(&path, expected_sample_rate, expected_duration);

        // 3. 调用你的 load_from_path 方法
        let result = AudioClip::load_from_path(&path);

        // 4. 清理测试文件 (无论测试成功与否，最好都清理，但在 panic 时可能会跳过)
        // 在实际项目中，推荐使用 `tempfile` crate 来自动清理
        let _ = std::fs::remove_file(&path);

        // 5. 验证结果
        assert!(result.is_ok(), "加载音频失败: {:?}", result.err());
        let clip = result.unwrap();

        println!(
            "Loaded Clip Info: Rate={}, Channels={}, Samples={}",
            clip.sample_rate,
            clip.channel_count,
            clip.samples.len()
        );

        // 验证采样率
        assert_eq!(clip.sample_rate, expected_sample_rate, "采样率不匹配");

        // 验证通道数 (我们的生成函数是单声道)
        assert_eq!(clip.channel_count, 1, "通道数不匹配");

        // 验证样本数量 (应该等于 采样率 * 时长)
        let expected_samples = (expected_sample_rate * expected_duration) as usize;
        assert_eq!(clip.samples.len(), expected_samples, "样本总数不匹配");

        // 验证样本数据范围 (f32 应该归一化到 -1.0 到 1.0 之间)
        for sample in clip.samples.iter().take(100) {
            assert!(
                *sample >= -1.0 && *sample <= 1.0,
                "样本数据超出归一化范围 [-1.0, 1.0]: {}",
                sample
            );
        }
    }

    #[test]
    fn test_load_non_existent_file() {
        let path = PathBuf::from("non_existent_audio_file.wav");
        let result = AudioClip::load_from_path(&path);

        assert!(result.is_err(), "读取不存在的文件应该报错");
    }
}
