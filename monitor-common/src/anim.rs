//! Animation system with keyframes and interpolation
//!
//! Ported from prpr/src/core/anim.rs
//! Provides keyframe-based animation for chart elements.

use crate::tween::{BezierTween, TweenFunction, TweenId, Tweenable, TWEEN_FUNCTIONS};
use serde::{Deserialize, Serialize};

/// A keyframe in an animation
#[derive(Clone, Serialize, Deserialize)]
pub struct Keyframe<T> {
    pub time: f32,
    pub value: T,
    pub tween: TweenId,
    /// Optional bezier control points for custom easing
    pub bezier: Option<BezierTween>,
}

impl<T> Keyframe<T> {
    pub fn new(time: f32, value: T, tween: TweenId) -> Self {
        Self {
            time,
            value,
            tween,
            bezier: None,
        }
    }

    pub fn with_bezier(time: f32, value: T, p1: (f32, f32), p2: (f32, f32)) -> Self {
        Self {
            time,
            value,
            tween: 2, // Linear as fallback
            bezier: Some(BezierTween::new(p1, p2)),
        }
    }

    /// Get the eased value for progress t in [0, 1]
    pub fn ease(&self, t: f32) -> f32 {
        if let Some(ref bezier) = self.bezier {
            bezier.y(t)
        } else {
            TWEEN_FUNCTIONS[self.tween as usize](t)
        }
    }
}

/// Keyframe-based animation
///
/// The tween function is taken from the first keyframe of each interval.
#[derive(Clone, Serialize, Deserialize)]
pub struct Anim<T: Tweenable> {
    pub keyframes: Vec<Keyframe<T>>,
    time: f32,
    cursor: usize,
}

impl<T: Tweenable> Default for Anim<T> {
    fn default() -> Self {
        Self {
            keyframes: Vec::new(),
            time: 0.0,
            cursor: 0,
        }
    }
}

impl<T: Tweenable> Anim<T> {
    pub fn new(keyframes: Vec<Keyframe<T>>) -> Self {
        Self {
            keyframes,
            time: 0.0,
            cursor: 0,
        }
    }

    /// Create an animation with a fixed (constant) value
    pub fn fixed(value: T) -> Self {
        Self {
            keyframes: vec![Keyframe::new(0.0, value, 0)], // tween 0 = hold
            time: 0.0,
            cursor: 0,
        }
    }

    /// Check if animation has no keyframes
    pub fn is_empty(&self) -> bool {
        self.keyframes.is_empty()
    }

    /// Check if cursor is at the last keyframe
    pub fn is_finished(&self) -> bool {
        self.cursor + 1 >= self.keyframes.len()
    }

    /// Get current time
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Set current time and update cursor position
    pub fn set_time(&mut self, time: f32) {
        if self.keyframes.is_empty() || time == self.time {
            self.time = time;
            return;
        }

        // Move cursor forward
        while let Some(kf) = self.keyframes.get(self.cursor + 1) {
            if kf.time > time {
                break;
            }
            self.cursor += 1;
        }

        // Move cursor backward
        while self.cursor != 0 && self.keyframes[self.cursor].time > time {
            self.cursor -= 1;
        }

        self.time = time;
    }

    /// Get current interpolated value, if any keyframes exist
    pub fn now_opt(&self) -> Option<T> {
        if self.keyframes.is_empty() {
            return None;
        }

        // At or past last keyframe
        if self.cursor == self.keyframes.len() - 1 {
            return Some(self.keyframes[self.cursor].value.clone());
        }

        // Interpolate between two keyframes
        let kf1 = &self.keyframes[self.cursor];
        let kf2 = &self.keyframes[self.cursor + 1];
        let t = (self.time - kf1.time) / (kf2.time - kf1.time);
        let eased_t = kf1.ease(t.clamp(0.0, 1.0));

        Some(T::tween(&kf1.value, &kf2.value, eased_t))
    }

    /// Apply a transformation to all keyframe values
    pub fn map_values<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> T,
    {
        for kf in &mut self.keyframes {
            kf.value = f(&kf.value);
        }
    }
}

impl<T: Tweenable + Default> Anim<T> {
    /// Get current value, or default if no keyframes
    pub fn now(&self) -> T {
        self.now_opt().unwrap_or_default()
    }
}

/// Type alias for f32 animation
pub type AnimFloat = Anim<f32>;

/// Animation for 2D vectors
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AnimVector {
    pub x: AnimFloat,
    pub y: AnimFloat,
}

impl AnimVector {
    pub fn new(x: AnimFloat, y: AnimFloat) -> Self {
        Self { x, y }
    }

    pub fn fixed(x: f32, y: f32) -> Self {
        Self {
            x: AnimFloat::fixed(x),
            y: AnimFloat::fixed(y),
        }
    }

    pub fn set_time(&mut self, time: f32) {
        self.x.set_time(time);
        self.y.set_time(time);
    }

    pub fn now(&self) -> (f32, f32) {
        (self.x.now(), self.y.now())
    }

    pub fn now_with_default(&self, def_x: f32, def_y: f32) -> (f32, f32) {
        (
            self.x.now_opt().unwrap_or(def_x),
            self.y.now_opt().unwrap_or(def_y),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_anim() {
        let anim = AnimFloat::fixed(42.0);
        assert_eq!(anim.now(), 42.0);
    }

    #[test]
    fn test_interpolation() {
        let mut anim = AnimFloat::new(vec![
            Keyframe::new(0.0, 0.0, 2), // Linear
            Keyframe::new(1.0, 100.0, 2),
        ]);

        anim.set_time(0.0);
        assert_eq!(anim.now(), 0.0);

        anim.set_time(0.5);
        assert!((anim.now() - 50.0).abs() < 0.001);

        anim.set_time(1.0);
        assert_eq!(anim.now(), 100.0);
    }

    #[test]
    fn test_quad_easing() {
        let mut anim = AnimFloat::new(vec![
            Keyframe::new(0.0, 0.0, 6), // QuadIn
            Keyframe::new(1.0, 100.0, 0),
        ]);

        anim.set_time(0.5);
        // QuadIn at 0.5 = 0.25, so value should be 25
        assert!((anim.now() - 25.0).abs() < 0.1);
    }
}
