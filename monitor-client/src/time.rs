//! Time manager for wall-clock to game-time synchronization.
//! Ported from `phira/prpr/src/time.rs`.

/// Manages the mapping from wall-clock (`performance.now()`) to game time,
/// supporting pause, resume, and seek operations.
pub struct TimeManager {
    /// Wall-clock time (seconds) corresponding to game time 0.
    start_time: f64,
    /// If paused, stores the wall-clock time at which the pause began.
    pause_time: Option<f64>,
}

impl TimeManager {
    pub fn new() -> Self {
        Self {
            start_time: Self::real_time_secs(),
            pause_time: None,
        }
    }

    /// Current wall-clock time in seconds (via `performance.now()`).
    pub fn real_time_secs() -> f64 {
        web_sys::window().unwrap().performance().unwrap().now() / 1000.0
    }

    /// Current game time in seconds.
    pub fn now(&self) -> f32 {
        let wall = self.pause_time.unwrap_or_else(Self::real_time_secs);
        (wall - self.start_time) as f32
    }

    pub fn paused(&self) -> bool {
        self.pause_time.is_some()
    }

    pub fn pause(&mut self) {
        if self.pause_time.is_none() {
            self.pause_time = Some(Self::real_time_secs());
        }
    }

    pub fn resume(&mut self) {
        if let Some(pt) = self.pause_time.take() {
            self.start_time += Self::real_time_secs() - pt;
        }
    }

    /// Jump game time to `pos` (in seconds).
    pub fn seek_to(&mut self, pos: f64) {
        let wall = self.pause_time.unwrap_or_else(Self::real_time_secs);
        self.start_time = wall - pos;
    }

    /// Reset to game time 0.
    pub fn reset(&mut self) {
        self.start_time = Self::real_time_secs();
        self.pause_time = None;
    }
}
