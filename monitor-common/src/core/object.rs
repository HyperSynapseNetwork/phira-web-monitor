//! Object transform animations
//!
//! Ported from prpr/src/core/object.rs
//! Provides transform animations for chart elements (notes, judge lines).

use super::anim::{AnimFloat, AnimVector};
use super::{Matrix, Vector};
use nalgebra::Rotation2;
use serde::{Deserialize, Serialize};

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
        self.alpha.is_default()
            && self.scale.x.is_default()
            && self.scale.y.is_default()
            && self.rotation.is_default()
            && self.translation.x.is_default()
            && self.translation.y.is_default()
    }

    /// Set time for all animations
    pub fn set_time(&mut self, time: f32) {
        self.alpha.set_time(time);
        self.scale.set_time(time);
        self.rotation.set_time(time);
        self.translation.set_time(time);
    }

    /// Check if all animations have finished
    pub fn dead(&self) -> bool {
        self.alpha.dead()
            && self.scale.x.dead()
            && self.scale.y.dead()
            && self.rotation.dead()
            && self.translation.x.dead()
            && self.translation.y.dead()
    }

    pub fn now(&self, aspect_ratio: f32) -> Matrix {
        self.now_rotation()
            .append_translation(&self.now_translation(aspect_ratio))
    }

    #[inline]
    pub fn now_rotation(&self) -> Matrix {
        Rotation2::new(self.rotation.now().to_radians()).to_homogeneous()
    }

    pub fn new_rotation_wrt_point(rot: Rotation2<f32>, pt: Vector) -> Matrix {
        let translation_back = Matrix::new_translation(&pt);
        let translation_to = Matrix::new_translation(&-pt);
        translation_back * rot.to_homogeneous() * translation_to
    }

    #[inline]
    pub fn now_translation(&self, aspect_ratio: f32) -> Vector {
        let mut tr = self.translation.now();
        tr.y *= aspect_ratio;
        tr
    }

    #[inline]
    pub fn now_alpha(&self) -> f32 {
        self.alpha.now_opt().unwrap_or(1.0).max(0.0)
    }

    #[inline]
    pub fn now_scale(&self, ct: Vector) -> Matrix {
        let scale = self.scale.now_with_default(1.0, 1.0);
        Matrix::new_translation(&-ct)
            .append_nonuniform_scaling(&scale)
            .append_translation(&ct)
    }

    // /// Get current scale (defaults to 1.0 if not animated)
    // pub fn now_scale(&self) -> Vector {
    //     self.scale.now_with_default(1.0, 1.0)
    // }

    // /// Build a rotation matrix
    // pub fn rotation_matrix(&self) -> Matrix {
    //     let angle = self.now_rotation_rad();
    //     let cos = angle.cos();
    //     let sin = angle.sin();
    //     Matrix::new(cos, -sin, 0.0, sin, cos, 0.0, 0.0, 0.0, 1.0)
    // }

    // /// Build a translation matrix
    // pub fn translation_matrix(&self, aspect_ratio: f32) -> Matrix {
    //     let (x, y) = self.now_translation();
    //     Matrix::new(1.0, 0.0, x, 0.0, 1.0, y / aspect_ratio, 0.0, 0.0, 1.0)
    // }

    // /// Build a scale matrix around a center point
    // pub fn scale_matrix(&self, center: (f32, f32)) -> Matrix {
    //     let (sx, sy) = self.now_scale();
    //     let (cx, cy) = center;
    //     Matrix::new(
    //         sx,
    //         0.0,
    //         cx * (1.0 - sx),
    //         0.0,
    //         sy,
    //         cy * (1.0 - sy),
    //         0.0,
    //         0.0,
    //         1.0,
    //     )
    // }

    // /// Build combined transform matrix
    // pub fn transform_matrix(&self, aspect_ratio: f32) -> Matrix {
    //     self.rotation_matrix() * self.translation_matrix(aspect_ratio)
    // }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_object() {
        let obj = Object::default();
        assert!(obj.is_default());
        assert_eq!(obj.now_alpha(), 1.0);
    }

    #[test]
    fn test_rotation_matrix() {
        let obj = Object::default();
        let mat = obj.now_rotation();
        // Default rotation is 0, so matrix should be identity
        assert!((mat[(0, 0)] - 1.0).abs() < 0.001);
        assert!((mat[(1, 1)] - 1.0).abs() < 0.001);
    }
}
