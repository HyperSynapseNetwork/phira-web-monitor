pub type Point = nalgebra::Point2<f32>;
pub type Vector = nalgebra::Vector2<f32>;
pub type Matrix = nalgebra::Matrix3<f32>;

pub const NOTE_WIDTH_RATIO_BASE: f32 = 0.13175016;
pub const HEIGHT_RATIO: f32 = 0.83175;

pub const EPS: f32 = 1e-5;

mod anim;
pub use anim::{Anim, AnimFloat, AnimVector, Keyframe, TweenFn};

mod bpm;
pub use bpm::{BpmList, Triple};

mod object;
pub use object::{CtrlObject, Object};

mod tween;
pub use tween::{
    easing_from, BezierTween, ClampedTween, StaticTween, TweenFunction, TweenId, TweenMajor,
    TweenMinor, Tweenable, TWEEN_FUNCTIONS,
};

mod color;
pub use color::{colors, Color};

mod chart;
pub use chart::{
    Chart, ChartFormat, ChartInfo, ChartSettings, GifFrames, HitSound, HitSoundMap, JudgeLine,
    JudgeLineKind, Note, NoteKind, UIElement,
};

mod texture;
pub use texture::Texture;

mod audio;
pub use audio::AudioClip;
