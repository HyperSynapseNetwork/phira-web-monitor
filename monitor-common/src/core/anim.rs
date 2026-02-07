//! Animation system with keyframes and interpolation
//!
//! Ported from prpr/src/core/anim.rs
//! Provides keyframe-based animation for chart elements.

use super::tween::{BezierTween, ClampedTween, TweenFunction, TweenId, Tweenable, TWEEN_FUNCTIONS};
use super::Vector;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub enum TweenFn {
    TweenId(TweenId),
    Bezier(BezierTween),
    Clamped(ClampedTween),
}

/// A keyframe in an animation
#[derive(Clone, Serialize, Deserialize)]
pub struct Keyframe<T> {
    pub time: f32,
    pub value: T,
    pub tween: TweenFn,
}

impl<T> Keyframe<T> {
    pub fn new(time: f32, value: T, tween: TweenId) -> Self {
        Self {
            time,
            value,
            tween: TweenFn::TweenId(tween),
        }
    }

    pub fn with_bezier(time: f32, value: T, p1: (f32, f32), p2: (f32, f32)) -> Self {
        Self {
            time,
            value,
            tween: TweenFn::Bezier(BezierTween::new(p1, p2)),
        }
    }

    pub fn with_clamped(time: f32, value: T, tween: std::ops::Range<f32>, id: TweenId) -> Self {
        Self {
            time,
            value,
            tween: TweenFn::Clamped(ClampedTween::new(id, tween)),
        }
    }

    /// Get the eased value for progress t in [0, 1]
    pub fn ease(&self, t: f32) -> f32 {
        match self.tween {
            TweenFn::Bezier(ref bezier) => bezier.y(t),
            TweenFn::Clamped(ref clamped) => clamped.y(t),
            TweenFn::TweenId(tween) => TWEEN_FUNCTIONS[tween as usize](t),
        }
    }
}

/// Keyframe-based animation
///
/// The tween function is taken from the first keyframe of each interval.
#[derive(Clone, Serialize, Deserialize)]
pub struct Anim<T: Tweenable> {
    pub time: f32,
    pub keyframes: Vec<Keyframe<T>>,
    pub cursor: usize,
    pub next: Option<Box<Anim<T>>>,
}

impl<T: Tweenable> Default for Anim<T> {
    fn default() -> Self {
        Self {
            time: 0.0,
            keyframes: Vec::new(),
            cursor: 0,
            next: None,
        }
    }
}

impl<T: Tweenable> Anim<T> {
    pub fn new(keyframes: Vec<Keyframe<T>>) -> Self {
        Self {
            time: 0.0,
            keyframes,
            cursor: 0,
            next: None,
        }
    }

    /// Create an animation with a fixed (constant) value
    pub fn fixed(value: T) -> Self {
        Self {
            time: 0.0,
            keyframes: vec![Keyframe::new(0.0, value, 0)], // tween 0 = hold
            cursor: 0,
            next: None,
        }
    }

    pub fn is_default(&self) -> bool {
        self.keyframes.is_empty() && self.next.is_none()
    }

    pub fn chain(elements: Vec<Anim<T>>) -> Self {
        if elements.is_empty() {
            return Self::default();
        }
        let mut elements: Vec<_> = elements.into_iter().map(Box::new).collect();
        elements.last_mut().unwrap().next = None;
        while elements.len() > 1 {
            let last = elements.pop().unwrap();
            elements.last_mut().unwrap().next = Some(last);
        }
        *elements.into_iter().next().unwrap()
    }

    pub fn dead(&self) -> bool {
        self.cursor + 1 >= self.keyframes.len()
    }

    pub fn set_time(&mut self, time: f32) {
        if self.keyframes.is_empty() || time == self.time {
            self.time = time;
            return;
        }
        while let Some(kf) = self.keyframes.get(self.cursor + 1) {
            if kf.time > time {
                break;
            }
            self.cursor += 1;
        }
        while self.cursor != 0 && self.keyframes[self.cursor].time > time {
            self.cursor -= 1;
        }
        self.time = time;
        if let Some(next) = &mut self.next {
            next.set_time(time);
        }
    }

    fn now_opt_inner(&self) -> Option<T> {
        if self.keyframes.is_empty() {
            return None;
        }
        Some(if self.cursor == self.keyframes.len() - 1 {
            self.keyframes[self.cursor].value.clone()
        } else {
            let kf1 = &self.keyframes[self.cursor];
            let kf2 = &self.keyframes[self.cursor + 1];
            let t = (self.time - kf1.time) / (kf2.time - kf1.time);
            T::tween(&kf1.value, &kf2.value, kf1.ease(t))
        })
    }

    pub fn now_opt(&self) -> Option<T> {
        let Some(now) = self.now_opt_inner() else {
            return None;
        };
        Some(if let Some(next) = &self.next {
            T::add(&now, &next.now_opt().unwrap())
        } else {
            now
        })
    }

    pub fn map_value(&mut self, mut f: impl FnMut(T) -> T) {
        self.keyframes
            .iter_mut()
            .for_each(|it| it.value = f(it.value.clone()));
        if let Some(next) = &mut self.next {
            next.map_value(f);
        }
    }
}

impl<T: Tweenable + Default> Anim<T> {
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

    pub fn fixed(v: Vector) -> Self {
        Self {
            x: AnimFloat::fixed(v.x),
            y: AnimFloat::fixed(v.y),
        }
    }

    pub fn set_time(&mut self, time: f32) {
        self.x.set_time(time);
        self.y.set_time(time);
    }

    pub fn now(&self) -> Vector {
        Vector::new(self.x.now(), self.y.now())
    }

    pub fn now_with_default(&self, x: f32, y: f32) -> Vector {
        Vector::new(self.x.now_opt().unwrap_or(x), self.y.now_opt().unwrap_or(y))
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
