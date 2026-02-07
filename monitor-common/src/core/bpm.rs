//! BPM list for beat-to-time conversion
//!
//! Ported from prpr/src/core.rs
//! Converts between beat coordinates and time in seconds.
use serde::{Deserialize, Serialize};

/// `(i, n, d)` represents beat position: `i + n / d`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Triple(pub i32, pub u32, pub u32);

impl Triple {
    pub fn new(i: i32, n: u32, d: u32) -> Self {
        Self(i, n, d)
    }

    /// Convert to beats as f32
    pub fn beats(&self) -> f32 {
        self.0 as f32 + self.1 as f32 / self.2 as f32
    }
}

/// BPM list for beat-to-time conversion
///
/// Stores BPM changes and provides conversion between beats and seconds.
#[derive(Clone, Serialize, Deserialize)]
pub struct BpmList {
    /// (beats, time_seconds, bpm)
    elements: Vec<(f32, f32, f32)>,
    /// Cursor for binary search optimization
    cursor: usize,
}

impl Default for BpmList {
    fn default() -> Self {
        Self {
            elements: vec![(0.0, 0.0, 120.0)], // Default 120 BPM
            cursor: 0,
        }
    }
}

impl BpmList {
    /// Create a new BpmList from a list of (beats, bpm) pairs
    ///
    /// Calculates the time offset for each BPM change.
    pub fn new(ranges: Vec<(f32, f32)>) -> Self {
        if ranges.is_empty() {
            return Self::default();
        }

        let mut elements = Vec::with_capacity(ranges.len());
        let mut time = 0.0;
        let mut last_beats = 0.0;
        let mut last_bpm: Option<f32> = None;

        for (now_beats, bpm) in ranges {
            if let Some(prev_bpm) = last_bpm {
                // Time = beats_delta * seconds_per_beat
                // seconds_per_beat = 60 / bpm
                time += (now_beats - last_beats) * (60.0 / prev_bpm);
            }
            last_beats = now_beats;
            last_bpm = Some(bpm);
            elements.push((now_beats, time, bpm));
        }

        BpmList {
            elements,
            cursor: 0,
        }
    }

    /// Get the time in seconds for a given beat position
    pub fn time_at_beats(&mut self, beats: f32) -> f32 {
        self.seek_by_beats(beats);
        let (start_beats, time, bpm) = &self.elements[self.cursor];
        time + (beats - start_beats) * (60.0 / bpm)
    }

    /// Get the time in seconds for a Triple beat position
    pub fn time_at(&mut self, triple: &Triple) -> f32 {
        self.time_at_beats(triple.beats())
    }

    /// Get the beat position for a given time in seconds
    pub fn beats_at_time(&mut self, time: f32) -> f32 {
        self.seek_by_time(time);
        let (beats, start_time, bpm) = &self.elements[self.cursor];
        beats + (time - start_time) / (60.0 / bpm)
    }

    /// Move cursor to the segment containing the given beats
    fn seek_by_beats(&mut self, beats: f32) {
        // Forward
        while let Some(kf) = self.elements.get(self.cursor + 1) {
            if kf.0 > beats {
                break;
            }
            self.cursor += 1;
        }
        // Backward
        while self.cursor != 0 && self.elements[self.cursor].0 > beats {
            self.cursor -= 1;
        }
    }

    /// Move cursor to the segment containing the given time
    fn seek_by_time(&mut self, time: f32) {
        // Forward
        while let Some(kf) = self.elements.get(self.cursor + 1) {
            if kf.1 > time {
                break;
            }
            self.cursor += 1;
        }
        // Backward
        while self.cursor != 0 && self.elements[self.cursor].1 > time {
            self.cursor -= 1;
        }
    }

    /// Reset cursor to beginning
    pub fn reset(&mut self) {
        self.cursor = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_bpm() {
        let mut bpm = BpmList::new(vec![(0.0, 120.0)]); // 120 BPM = 0.5s per beat

        assert!((bpm.time_at_beats(0.0) - 0.0).abs() < 0.001);
        assert!((bpm.time_at_beats(1.0) - 0.5).abs() < 0.001);
        assert!((bpm.time_at_beats(4.0) - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_bpm_change() {
        // 0-2 beats at 120 BPM (1s), then 60 BPM
        let mut bpm = BpmList::new(vec![(0.0, 120.0), (2.0, 60.0)]);

        // At beat 2, time should be 1.0s
        assert!((bpm.time_at_beats(2.0) - 1.0).abs() < 0.001);
        // At beat 3, time should be 1.0 + 1.0 = 2.0s (60 BPM = 1s per beat)
        assert!((bpm.time_at_beats(3.0) - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_beats_at_time() {
        let mut bpm = BpmList::new(vec![(0.0, 120.0)]);

        assert!((bpm.beats_at_time(0.0) - 0.0).abs() < 0.001);
        assert!((bpm.beats_at_time(0.5) - 1.0).abs() < 0.001);
        assert!((bpm.beats_at_time(2.0) - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_triple() {
        let triple = Triple::new(1, 1, 2); // 1 + 1/2 = 1.5 beats
        assert!((triple.beats() - 1.5).abs() < 0.001);
    }
}
