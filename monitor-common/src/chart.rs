//! Chart data structures
//!
//! Simplified from prpr/src/core for the web monitor.
//! Contains only data definitions without rendering logic.

use crate::anim::{AnimFloat, AnimVector};
use crate::bpm::BpmList;
use crate::object::{CtrlObject, Object};
use serde::{Deserialize, Serialize};

// ============================================================================
// Note types
// ============================================================================

/// Type of note
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

/// A single note in the chart
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
}

impl Default for Note {
    fn default() -> Self {
        Self {
            object: Object::default(),
            kind: NoteKind::Click,
            time: 0.0,
            height: 0.0,
            speed: 1.0,
            above: true,
            multiple_hint: false,
            fake: false,
        }
    }
}

impl Note {
    pub fn new(kind: NoteKind, time: f32, height: f32) -> Self {
        Self {
            kind,
            time,
            height,
            ..Default::default()
        }
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

/// Type of judge line
#[derive(Clone, Default, Serialize, Deserialize)]
pub enum JudgeLineKind {
    #[default]
    Normal,
    Texture(String), // Texture path
    Text(String),    // Text content
}

/// A judge line containing notes
#[derive(Clone, Serialize, Deserialize)]
pub struct JudgeLine {
    /// Object transform animations
    pub object: Object,
    /// Control object for note animations
    pub ctrl_obj: CtrlObject,
    /// Kind of judge line
    pub kind: JudgeLineKind,
    /// Notes on this line
    pub notes: Vec<Note>,
    /// Speed animation
    pub speed: AnimFloat,
    /// Height animation (vertical position)
    pub height: AnimFloat,
    /// Incline animation (perspective tilt)
    pub incline: AnimFloat,
    /// Color animation (r, g, b packed or separate animations)
    pub color: AnimVector,
    /// Alpha animation
    pub alpha: AnimFloat,
    /// Parent line index (for attached lines)
    pub parent: Option<usize>,
    /// Whether to show line
    pub show_below: bool,
    /// Z-order for rendering
    pub z_order: i32,
}

impl Default for JudgeLine {
    fn default() -> Self {
        Self {
            object: Object::default(),
            ctrl_obj: CtrlObject::default(),
            kind: JudgeLineKind::Normal,
            notes: Vec::new(),
            speed: AnimFloat::default(),
            height: AnimFloat::default(),
            incline: AnimFloat::default(),
            color: AnimVector::default(),
            alpha: AnimFloat::default(),
            parent: None,
            show_below: true,
            z_order: 0,
        }
    }
}

impl JudgeLine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set time for all animations
    pub fn set_time(&mut self, time: f32) {
        self.object.set_time(time);
        self.speed.set_time(time);
        self.height.set_time(time);
        self.incline.set_time(time);
        self.color.set_time(time);
        self.alpha.set_time(time);
    }

    /// Get current alpha value
    pub fn now_alpha(&self) -> f32 {
        self.alpha.now_opt().unwrap_or(1.0).clamp(0.0, 1.0)
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

/// Chart settings
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ChartSettings {
    pub pe_alpha_extension: bool,
    pub hold_partial_cover: bool,
}

/// A complete chart
#[derive(Clone, Serialize, Deserialize)]
pub struct Chart {
    /// Offset in seconds (for sync adjustment)
    pub offset: f32,
    /// All judge lines
    pub lines: Vec<JudgeLine>,
    /// BPM list for beat-to-time conversion
    pub bpm_list: BpmList,
    /// Chart settings
    pub settings: ChartSettings,
}

impl Default for Chart {
    fn default() -> Self {
        Self {
            offset: 0.0,
            lines: Vec::new(),
            bpm_list: BpmList::default(),
            settings: ChartSettings::default(),
        }
    }
}

impl Chart {
    pub fn new(offset: f32, lines: Vec<JudgeLine>, bpm_list: BpmList) -> Self {
        Self {
            offset,
            lines,
            bpm_list,
            settings: ChartSettings::default(),
        }
    }

    /// Set time for all chart elements
    pub fn set_time(&mut self, time: f32) {
        for line in &mut self.lines {
            line.set_time(time);
            for note in &mut line.notes {
                note.set_time(time);
            }
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
