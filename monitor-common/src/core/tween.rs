//! Tween and easing functions
//!
//! Ported from prpr/src/core/tween.rs
//! Provides interpolation functions for chart animations.

use super::{Color, Vector};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;
use std::ops::Range;

/// Type alias for tween function identifier
pub type TweenId = u8;

// ============================================================================
// Core easing functions (In variants)
// ============================================================================

#[inline]
fn sine(x: f32) -> f32 {
    1. - ((x * PI) / 2.).cos()
}

#[inline]
fn quad(x: f32) -> f32 {
    x * x
}

#[inline]
fn cubic(x: f32) -> f32 {
    x * x * x
}

#[inline]
fn quart(x: f32) -> f32 {
    x * x * x * x
}

#[inline]
fn quint(x: f32) -> f32 {
    x * x * x * x * x
}

#[inline]
fn expo(x: f32) -> f32 {
    (2.0_f32).powf(10. * (x - 1.))
}

#[inline]
fn circ(x: f32) -> f32 {
    1. - (1. - x * x).sqrt()
}

#[inline]
fn back(x: f32) -> f32 {
    const C1: f32 = 1.70158;
    const C3: f32 = C1 + 1.;
    (C3 * x - C1) * x * x
}

#[inline]
fn elastic(x: f32) -> f32 {
    const C4: f32 = (2. * PI) / 3.;
    -((2.0_f32).powf(10. * x - 10.) * ((x * 10. - 10.75) * C4).sin())
}

#[inline]
fn bounce(x: f32) -> f32 {
    const N1: f32 = 7.5625;
    const D1: f32 = 2.75;

    let x = 1. - x;
    1. - (if x < 1. / D1 {
        N1 * x.powi(2)
    } else if x < 2. / D1 {
        N1 * (x - 1.5 / D1).powi(2) + 0.75
    } else if x < 2.5 / D1 {
        N1 * (x - 2.25 / D1).powi(2) + 0.9375
    } else {
        N1 * (x - 2.625 / D1).powi(2) + 0.984375
    })
}

// ============================================================================
// Out and InOut variants via macros
// ============================================================================

macro_rules! ease_out {
    ($fn:ident, $x:expr) => {
        1. - $fn(1. - $x)
    };
}

macro_rules! ease_in_out {
    ($fn:ident, $x:expr) => {{
        let x = $x * 2.;
        if x < 1. {
            $fn(x) / 2.
        } else {
            1. - $fn(2. - x) / 2.
        }
    }};
}

// ============================================================================
// Static tween function table
// ============================================================================

/// All 33 predefined easing functions
/// Index: 0=hold, 1=constant, 2=linear, then groups of 3 (In, Out, InOut) for each type
#[rustfmt::skip]
pub static TWEEN_FUNCTIONS: [fn(f32) -> f32; 33] = [
    |_| 0.,             // 0: Hold
    |_| 1.,             // 1: Constant
    |x| x,              // 2: Linear
    // Sine
    |x| sine(x),                        // 3: SineIn
    |x| ease_out!(sine, x),             // 4: SineOut
    |x| ease_in_out!(sine, x),          // 5: SineInOut
    // Quad
    |x| quad(x),                        // 6: QuadIn
    |x| ease_out!(quad, x),             // 7: QuadOut
    |x| ease_in_out!(quad, x),          // 8: QuadInOut
    // Cubic
    |x| cubic(x),                       // 9: CubicIn
    |x| ease_out!(cubic, x),            // 10: CubicOut
    |x| ease_in_out!(cubic, x),         // 11: CubicInOut
    // Quart
    |x| quart(x),                       // 12: QuartIn
    |x| ease_out!(quart, x),            // 13: QuartOut
    |x| ease_in_out!(quart, x),         // 14: QuartInOut
    // Quint
    |x| quint(x),                       // 15: QuintIn
    |x| ease_out!(quint, x),            // 16: QuintOut
    |x| ease_in_out!(quint, x),         // 17: QuintInOut
    // Expo
    |x| expo(x),                        // 18: ExpoIn
    |x| ease_out!(expo, x),             // 19: ExpoOut
    |x| ease_in_out!(expo, x),          // 20: ExpoInOut
    // Circ
    |x| circ(x),                        // 21: CircIn
    |x| ease_out!(circ, x),             // 22: CircOut
    |x| ease_in_out!(circ, x),          // 23: CircInOut
    // Back
    |x| back(x),                        // 24: BackIn
    |x| ease_out!(back, x),             // 25: BackOut
    |x| ease_in_out!(back, x),          // 26: BackInOut
    // Elastic
    |x| elastic(x),                     // 27: ElasticIn
    |x| ease_out!(elastic, x),          // 28: ElasticOut
    |x| ease_in_out!(elastic, x),       // 29: ElasticInOut
    // Bounce
    |x| bounce(x),                      // 30: BounceIn
    |x| ease_out!(bounce, x),           // 31: BounceOut
    |x| ease_in_out!(bounce, x),        // 32: BounceInOut
];

// ============================================================================
// Tween types
// ============================================================================

/// Major tween category
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TweenMajor {
    Plain = 0,
    Sine = 1,
    Quad = 2,
    Cubic = 3,
    Quart = 4,
    Quint = 5,
    Expo = 6,
    Circ = 7,
    Back = 8,
    Elastic = 9,
    Bounce = 10,
}

/// Minor tween variant (In/Out/InOut)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TweenMinor {
    In = 0,
    Out = 1,
    InOut = 2,
}

/// Convert major/minor pair to tween ID
pub const fn easing_from(major: TweenMajor, minor: TweenMinor) -> TweenId {
    major as u8 * 3 + minor as u8
}

// ============================================================================
// Tween trait and implementations
// ============================================================================

/// A tween/easing function
pub trait TweenFunction {
    /// Compute the eased value for input x in [0, 1]
    fn y(&self, x: f32) -> f32;
}

/// A static tween using one of the 33 predefined functions
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StaticTween(pub TweenId);

impl TweenFunction for StaticTween {
    fn y(&self, x: f32) -> f32 {
        TWEEN_FUNCTIONS[self.0 as usize](x)
    }
}

/// A clamped tween that uses only a portion of the easing curve
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClampedTween(pub TweenId, pub Range<f32>, pub Range<f32>);

impl ClampedTween {
    pub fn new(tween: TweenId, range: Range<f32>) -> Self {
        let f = TWEEN_FUNCTIONS[tween as usize];
        let y_range = f(range.start)..f(range.end);
        Self(tween, range, y_range)
    }
}

impl TweenFunction for ClampedTween {
    fn y(&self, x: f32) -> f32 {
        (TWEEN_FUNCTIONS[self.0 as usize](f32::tween(&self.1.start, &self.1.end, x)) - self.2.start)
            / (self.2.end - self.2.start)
    }
}

// ============================================================================
// Bezier tween (cubic bezier easing)
// ============================================================================

const SAMPLE_TABLE_SIZE: usize = 21;
const SAMPLE_STEP: f32 = 1. / (SAMPLE_TABLE_SIZE - 1) as f32;
const NEWTON_ITERATIONS: usize = 4;
const NEWTON_MIN_STEP: f32 = 1e-3;
const SUBDIVISION_PRECISION: f32 = 1e-7;
const SUBDIVISION_MAX_ITERATION: usize = 10;
const SLOPE_EPS: f32 = 1e-7;

/// Cubic bezier easing function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BezierTween {
    sample_table: [f32; SAMPLE_TABLE_SIZE],
    pub p1: (f32, f32),
    pub p2: (f32, f32),
}

impl BezierTween {
    pub fn new(p1: (f32, f32), p2: (f32, f32)) -> Self {
        let mut sample_table = [0.0; SAMPLE_TABLE_SIZE];
        for i in 0..SAMPLE_TABLE_SIZE {
            sample_table[i] = Self::sample(p1.0, p2.0, i as f32 * SAMPLE_STEP);
        }
        Self {
            sample_table,
            p1,
            p2,
        }
    }

    #[inline]
    fn coefficients(x1: f32, x2: f32) -> (f32, f32, f32) {
        ((x1 - x2) * 3. + 1., x2 * 3. - x1 * 6., x1 * 3.)
    }

    #[inline]
    fn sample(x1: f32, x2: f32, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(x1, x2);
        ((a * t + b) * t + c) * t
    }

    #[inline]
    fn slope(x1: f32, x2: f32, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(x1, x2);
        (a * 3. * t + b * 2.) * t + c
    }

    fn newton_raphson_iterate(x: f32, mut t: f32, x1: f32, x2: f32) -> f32 {
        for _ in 0..NEWTON_ITERATIONS {
            let slope = Self::slope(x1, x2, t);
            if slope <= SLOPE_EPS {
                return t;
            }
            let diff = Self::sample(x1, x2, t) - x;
            t -= diff / slope;
        }
        t
    }

    fn binary_subdivide(x: f32, mut l: f32, mut r: f32, x1: f32, x2: f32) -> f32 {
        let mut t = (l + r) / 2.;
        for _ in 0..SUBDIVISION_MAX_ITERATION {
            let diff = Self::sample(x1, x2, t) - x;
            if diff.abs() <= SUBDIVISION_PRECISION {
                break;
            }
            if diff > 0. {
                r = t;
            } else {
                l = t;
            }
            t = (l + r) / 2.;
        }
        t
    }

    pub fn t_for_x(&self, x: f32) -> f32 {
        if x == 0. || x == 1. {
            return x;
        }
        let id = (x / SAMPLE_STEP) as usize;
        let id = id.min(SAMPLE_TABLE_SIZE - 2);
        let dist =
            (x - self.sample_table[id]) / (self.sample_table[id + 1] - self.sample_table[id]);
        let init_t = SAMPLE_STEP * (id as f32 + dist);
        match Self::slope(self.p1.0, self.p2.0, init_t) {
            y if y <= SLOPE_EPS => init_t,
            y if y >= NEWTON_MIN_STEP => {
                Self::newton_raphson_iterate(x, init_t, self.p1.0, self.p2.0)
            }
            _ => Self::binary_subdivide(
                x,
                SAMPLE_STEP * id as f32,
                SAMPLE_STEP * (id + 1) as f32,
                self.p1.0,
                self.p2.0,
            ),
        }
    }
}

impl TweenFunction for BezierTween {
    fn y(&self, x: f32) -> f32 {
        Self::sample(self.p1.1, self.p2.1, self.t_for_x(x))
    }
}

// ============================================================================
// Tweenable trait for interpolation
// ============================================================================

/// Trait for types that can be interpolated
pub trait Tweenable: Clone {
    fn tween(a: &Self, b: &Self, t: f32) -> Self;
    fn add(_x: &Self, _y: &Self) -> Self {
        unimplemented!()
    }
}

impl Tweenable for f32 {
    fn tween(a: &Self, b: &Self, t: f32) -> Self {
        a + (b - a) * t
    }
}

impl Tweenable for Vector {
    fn tween(x: &Self, y: &Self, t: f32) -> Self {
        Vector::new(f32::tween(&x.x, &y.x, t), f32::tween(&x.y, &y.y, t))
    }

    fn add(x: &Self, y: &Self) -> Self {
        Vector::new(x.x + y.x, x.y + y.y)
    }
}

impl Tweenable for Color {
    fn tween(x: &Self, y: &Self, t: f32) -> Self {
        Self::new(
            f32::tween(&x.r, &y.r, t),
            f32::tween(&x.g, &y.g, t),
            f32::tween(&x.b, &y.b, t),
            f32::tween(&x.a, &y.a, t),
        )
    }
}

impl Tweenable for String {
    fn tween(x: &Self, y: &Self, t: f32) -> Self {
        if x.contains("%P%") && y.contains("%P%") {
            let x = x.replace("%P%", "");
            let y = y.replace("%P%", "");
            if t >= 1. {
                y
            } else if t <= 0. {
                x
            } else {
                let x: f32 = x.parse().unwrap_or(0.0);
                let y: f32 = y.parse().unwrap_or(0.0);
                let value = x + t * (y - x);
                if x.fract() == 0.0 && y.fract() == 0.0 {
                    format!("{:.0}", value)
                } else {
                    format!("{:.3}", value)
                }
            }
        } else if x.is_empty() && y.is_empty() {
            Self::new()
        } else if y.is_empty() {
            let x = if x.contains("%P%") {
                x.replace("%P%", "")
            } else {
                x.to_string()
            };
            Self::tween(y, &x, 1. - t)
        } else if x.is_empty() {
            let chars = y.chars().collect::<Vec<_>>();
            chars[..(t * chars.len() as f32).round() as usize]
                .iter()
                .collect()
        } else {
            let x_len = x.chars().count();
            let y_len = y.chars().count();
            if y.starts_with(x) {
                // x in y
                let take_num = ((y_len - x_len) as f32 * t).floor() as usize + x_len;
                let mut text = x.clone();
                text.push_str(
                    &y.chars()
                        .skip(x_len)
                        .take(take_num - x_len)
                        .collect::<String>(),
                );
                text
            } else if x.starts_with(y) {
                // y in x
                let take_num = ((x_len - y_len) as f32 * (1. - t)).round() as usize + y_len;
                let mut text = y.clone();
                text.push_str(
                    &x.chars()
                        .skip(y_len)
                        .take(take_num - y_len)
                        .collect::<String>(),
                );
                text
            } else {
                if x.contains("%P%") {
                    x.replace("%P%", "")
                } else {
                    x.clone()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear() {
        let tween = StaticTween(2); // Linear
        assert_eq!(tween.y(0.0), 0.0);
        assert_eq!(tween.y(0.5), 0.5);
        assert_eq!(tween.y(1.0), 1.0);
    }

    #[test]
    fn test_quad_in() {
        let tween = StaticTween(6); // QuadIn
        assert_eq!(tween.y(0.0), 0.0);
        assert!((tween.y(0.5) - 0.25).abs() < 0.001);
        assert_eq!(tween.y(1.0), 1.0);
    }

    #[test]
    fn test_bezier() {
        let tween = BezierTween::new((0.25, 0.1), (0.25, 1.0));
        assert!((tween.y(0.0) - 0.0).abs() < 0.001);
        assert!((tween.y(1.0) - 1.0).abs() < 0.001);
    }
}
