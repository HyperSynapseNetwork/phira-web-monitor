//! Chart data structures
//!
//! Simplified from prpr/src/core for the web monitor.
//! Contains only data definitions without rendering logic.

use super::{Anim, AnimFloat, AudioClip, BpmList, Color, CtrlObject, Object, Texture};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Note types
// ============================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NoteKind {
    Click,
    Hold { end_time: f32, end_height: f32 },
    Flick,
    Drag,
}

impl NoteKind {
    /// Render order (lower = rendered first, appears behind)
    pub fn order(&self) -> i8 {
        match self {
            Self::Hold { .. } => 0,
            Self::Drag => 1,
            Self::Click => 2,
            Self::Flick => 3,
        }
    }

    pub fn is_hold(&self) -> bool {
        matches!(self, Self::Hold { .. })
    }
}

impl Default for NoteKind {
    fn default() -> Self {
        Self::Click
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub enum JudgeStatus {
    #[default]
    NotJudged,
    PreJudge,
    Judged,
    Hold(bool, f32, f32, bool, f32), // perfect, at, diff, pre-judge, up-time
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum Judgement {
    Perfect,
    Good,
    Bad,
    Miss,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Note {
    /// Object transform animations
    pub object: Object,
    /// Type of note
    pub kind: NoteKind,
    /// Time when note should be hit (seconds)
    pub time: f32,
    /// Height on the judge line (y-position relative to line)
    pub height: f32,
    /// Speed multiplier
    pub speed: f32,
    /// Whether note appears above the judge line
    pub above: bool,
    /// Whether this note is part of a chord (multiple notes at same time)
    pub multiple_hint: bool,
    /// Whether this is a fake note (doesn't count for score)
    pub fake: bool,
    /// Index of the hitsound in the chart's audio clips
    pub hitsound: Option<HitSound>,
    /// Judge status
    #[serde(skip)]
    pub judge: JudgeStatus,
}

impl Default for Note {
    fn default() -> Self {
        Self {
            object: Object::default(),
            kind: NoteKind::default(),
            time: 0.,
            height: 0.,
            speed: 1.,
            above: true,
            multiple_hint: false,
            fake: false,
            hitsound: None,
            judge: JudgeStatus::NotJudged,
        }
    }
}

impl Note {
    pub fn new(kind: NoteKind, time: f32, height: f32) -> Self {
        Self {
            object: Object::default(),
            kind,
            time,
            height,
            speed: 1.,
            above: true,
            multiple_hint: false,
            fake: false,
            hitsound: None,
            judge: JudgeStatus::NotJudged,
        }
    }

    pub fn rotation(&self, line: &JudgeLine) -> f32 {
        line.object.rotation.now() + if self.above { 0. } else { 180. }
    }

    pub fn plain(&self) -> bool {
        !self.fake
            && !matches!(self.kind, NoteKind::Hold { .. })
            && self.object.translation.y.keyframes.len() <= 1
        // && self.ctrl_obj.is_default()
    }

    /// Set time for the note's animations
    pub fn set_time(&mut self, time: f32) {
        self.object.set_time(time);
    }

    /// Get end time for Hold notes
    pub fn end_time(&self) -> f32 {
        match &self.kind {
            NoteKind::Hold { end_time, .. } => *end_time,
            _ => self.time,
        }
    }
}

// ============================================================================
// Judge Line types
// ============================================================================

#[derive(Clone, Serialize, Deserialize)]
pub struct GifFrames {
    /// time of each frame in milliseconds
    pub frames: Vec<(u128, Texture)>,
    /// milliseconds
    pub total_time: u128,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum UIElement {
    Pause = 1,
    ComboNumber = 2,
    Combo = 3,
    Score = 4,
    Bar = 5,
    Name = 6,
    Level = 7,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub enum JudgeLineKind {
    #[default]
    Normal,
    Texture(Texture, String),
    TextureGif(Anim<f32>, GifFrames, String),
    Text(Anim<String>),
    Paint(Anim<f32>),
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct JudgeLine {
    /// Object transform animations
    pub object: Object,
    /// Control object for note animations
    pub ctrl_obj: CtrlObject,
    /// Kind of judge line
    pub kind: JudgeLineKind,
    /// Height Animation
    pub height: AnimFloat,
    /// Incline animation (perspective tilt)
    pub incline: AnimFloat,
    /// Color animation (r, g, b packed or separate animations)
    pub color: Anim<Color>,
    /// Notes on this line
    pub notes: Vec<Note>,
    /// Parent line index (for attached lines)
    pub parent: Option<usize>,
    /// Z-order for rendering
    pub z_index: i32,
    /// Whether to show notes below the line, here below is defined in the time axis, which means the note should already be judged
    pub show_below: bool,
    // UI element to attach
    pub attach_ui: Option<UIElement>,
}

impl JudgeLine {
    /// Set time for all animations
    pub fn set_time(&mut self, time: f32) {
        self.object.set_time(time);
        self.height.set_time(time);
        self.incline.set_time(time);
        self.color.set_time(time);
        for note in &mut self.notes {
            note.set_time(time);
        }
    }

    /// Get current height
    pub fn now_height(&self) -> f32 {
        self.height.now()
    }

    /// Get note count
    pub fn note_count(&self) -> usize {
        self.notes.iter().filter(|n| !n.fake).count()
    }
}

// ============================================================================
// Chart
// ============================================================================

/// Chart format
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
#[serde(rename_all = "lowercase")]
pub enum ChartFormat {
    Rpe = 0,
    Pec,
    Pgr,
    Pbc,
}

/// Chart information
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct ChartInfo {
    pub id: Option<i32>,
    pub uploader: Option<i32>,

    pub name: String,
    pub difficulty: f32,
    pub level: String,
    pub charter: String,
    pub composer: String,
    pub illustrator: String,

    pub chart: String,
    pub format: Option<ChartFormat>,
    pub music: String,
    pub illustration: String,
    pub unlock_video: Option<String>,

    pub preview_start: f32,
    pub preview_end: Option<f32>,
    pub aspect_ratio: f32,
    pub background_dim: f32,
    pub line_length: f32,
    pub offset: f32,
    pub tip: Option<String>,
    pub tags: Vec<String>,

    pub intro: String,

    pub hold_partial_cover: bool,
    pub note_uniform_scale: bool,

    pub created: Option<DateTime<Utc>>,
    pub updated: Option<DateTime<Utc>>,
    pub chart_updated: Option<DateTime<Utc>>,
}

impl Default for ChartInfo {
    fn default() -> Self {
        Self {
            id: None,
            uploader: None,

            name: "UK".to_string(),
            difficulty: 10.,
            level: "UK Lv.10".to_string(),
            charter: "UK".to_string(),
            composer: "UK".to_string(),
            illustrator: "UK".to_string(),

            chart: "chart.json".to_string(),
            format: None,
            music: "song.mp3".to_string(),
            illustration: "background.png".to_string(),
            unlock_video: None,

            preview_start: 0.,
            preview_end: None,
            aspect_ratio: 16. / 9.,
            background_dim: 0.6,
            line_length: 6.,
            offset: 0.,
            tip: None,
            tags: Vec::new(),

            intro: String::new(),

            hold_partial_cover: false,
            note_uniform_scale: false,

            created: None,
            updated: None,
            chart_updated: None,
        }
    }
}

/// Chart settings
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ChartSettings {
    pub pe_alpha_extension: bool,
    pub hold_partial_cover: bool,
}

/// HitSound
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitSound {
    Click,
    Drag,
    Flick,
    Custom(String),
}

pub type HitSoundMap = HashMap<HitSound, AudioClip>;

/// A complete chart
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Chart {
    /// Music for the chart
    pub music: Option<AudioClip>,
    /// Offset in seconds (for sync adjustment)
    pub offset: f32,
    /// All judge lines
    pub lines: Vec<JudgeLine>,
    /// BPM list for beat-to-time conversion
    pub bpm_list: BpmList,
    /// Chart settings
    pub settings: ChartSettings,
    // pub extra: ChartExtra,
    /// Line order according to z-index, lines with attach_ui will be removed from this list
    ///
    /// Store the index of the line in z-index ascending order
    #[serde(skip)]
    pub order: Vec<usize>,
    // /// TODO: docs from RPE
    // pub attach_ui: [Option<usize>; 7],
    pub hitsounds: HitSoundMap,
}

impl Chart {
    pub fn new(offset: f32, lines: Vec<JudgeLine>, bpm_list: BpmList) -> Self {
        Self {
            music: None,
            offset,
            lines,
            bpm_list,
            ..Default::default()
        }
    }

    /// Set time for all chart elements
    pub fn set_time(&mut self, time: f32) {
        for line in &mut self.lines {
            line.set_time(time);
        }
    }

    /// Get total note count (excluding fake notes)
    pub fn note_count(&self) -> usize {
        self.lines.iter().map(|l| l.note_count()).sum()
    }

    /// Get total line count
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_kind_order() {
        assert!(
            NoteKind::Hold {
                end_time: 0.0,
                end_height: 0.0
            }
            .order()
                < NoteKind::Drag.order()
        );
        assert!(NoteKind::Drag.order() < NoteKind::Click.order());
        assert!(NoteKind::Click.order() < NoteKind::Flick.order());
    }

    #[test]
    fn test_chart_note_count() {
        let mut chart = Chart::default();
        let mut line = JudgeLine::default();
        line.notes.push(Note::new(NoteKind::Click, 1.0, 0.0));
        line.notes.push(Note::new(NoteKind::Click, 2.0, 0.0));
        let mut fake_note = Note::new(NoteKind::Click, 3.0, 0.0);
        fake_note.fake = true;
        line.notes.push(fake_note);
        chart.lines.push(line);

        assert_eq!(chart.note_count(), 2); // Fake notes not counted
    }
}
