//! Object transform animations
//!
//! Ported from prpr/src/core/object.rs
//! Provides transform animations for chart elements (notes, judge lines).

use crate::anim::{AnimFloat, AnimVector};
use serde::{Deserialize, Serialize};

/// 2D transformation matrix (3x3 homogeneous)
pub type Matrix = nalgebra::Matrix3<f32>;
pub type Vector = nalgebra::Vector2<f32>;

/// Describes animated transform of a chart element
///
/// Includes alpha, scale, rotation, and translation animations.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Object {
    pub alpha: AnimFloat,
    pub scale: AnimVector,
    /// Rotation in degrees
    pub rotation: AnimFloat,
    pub translation: AnimVector,
}

impl Object {
    /// Check if all animations are at default state
    pub fn is_default(&self) -> bool {
        self.alpha.is_empty()
            && self.scale.x.is_empty()
            && self.scale.y.is_empty()
            && self.rotation.is_empty()
            && self.translation.x.is_empty()
            && self.translation.y.is_empty()
    }

    /// Set time for all animations
    pub fn set_time(&mut self, time: f32) {
        self.alpha.set_time(time);
        self.scale.set_time(time);
        self.rotation.set_time(time);
        self.translation.set_time(time);
    }

    /// Check if all animations have finished
    pub fn is_finished(&self) -> bool {
        self.alpha.is_finished()
            && self.scale.x.is_finished()
            && self.scale.y.is_finished()
            && self.rotation.is_finished()
            && self.translation.x.is_finished()
            && self.translation.y.is_finished()
    }

    /// Get current alpha value (defaults to 1.0 if not animated)
    pub fn now_alpha(&self) -> f32 {
        self.alpha.now_opt().unwrap_or(1.0).max(0.0)
    }

    /// Get current rotation in radians
    pub fn now_rotation_rad(&self) -> f32 {
        self.rotation.now().to_radians()
    }

    /// Get current translation
    pub fn now_translation(&self) -> (f32, f32) {
        self.translation.now()
    }

    /// Get current scale (defaults to 1.0 if not animated)
    pub fn now_scale(&self) -> (f32, f32) {
        self.scale.now_with_default(1.0, 1.0)
    }

    /// Build a rotation matrix
    pub fn rotation_matrix(&self) -> Matrix {
        let angle = self.now_rotation_rad();
        let cos = angle.cos();
        let sin = angle.sin();
        Matrix::new(cos, -sin, 0.0, sin, cos, 0.0, 0.0, 0.0, 1.0)
    }

    /// Build a translation matrix
    pub fn translation_matrix(&self, aspect_ratio: f32) -> Matrix {
        let (x, y) = self.now_translation();
        Matrix::new(1.0, 0.0, x, 0.0, 1.0, y / aspect_ratio, 0.0, 0.0, 1.0)
    }

    /// Build a scale matrix around a center point
    pub fn scale_matrix(&self, center: (f32, f32)) -> Matrix {
        let (sx, sy) = self.now_scale();
        let (cx, cy) = center;
        Matrix::new(
            sx,
            0.0,
            cx * (1.0 - sx),
            0.0,
            sy,
            cy * (1.0 - sy),
            0.0,
            0.0,
            1.0,
        )
    }

    /// Build combined transform matrix
    pub fn transform_matrix(&self, aspect_ratio: f32) -> Matrix {
        self.rotation_matrix() * self.translation_matrix(aspect_ratio)
    }
}

/// Control object for note position animations along judge line
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct CtrlObject {
    pub alpha: AnimFloat,
    pub size: AnimFloat,
    pub pos: AnimFloat,
    pub y: AnimFloat,
}

impl CtrlObject {
    /// Set time using height (distance from judge line)
    pub fn set_height(&mut self, height: f32) {
        self.alpha.set_time(height);
        self.size.set_time(height);
        self.pos.set_time(height);
        self.y.set_time(height);
    }

    pub fn now_alpha(&self) -> f32 {
        self.alpha.now_opt().unwrap_or(1.0).max(0.0)
    }

    pub fn now_size(&self) -> f32 {
        self.size.now_opt().unwrap_or(1.0).max(0.0)
    }

    pub fn now_pos(&self) -> f32 {
        self.pos.now()
    }

    pub fn now_y(&self) -> f32 {
        self.y.now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_object() {
        let obj = Object::default();
        assert!(obj.is_default());
        assert_eq!(obj.now_alpha(), 1.0);
        assert_eq!(obj.now_scale(), (1.0, 1.0));
    }

    #[test]
    fn test_rotation_matrix() {
        let obj = Object::default();
        let mat = obj.rotation_matrix();
        // Default rotation is 0, so matrix should be identity
        assert!((mat[(0, 0)] - 1.0).abs() < 0.001);
        assert!((mat[(1, 1)] - 1.0).abs() < 0.001);
    }
}
