//! RPE chart format parser
//!
//! Simplified from prpr/src/parse/rpe.rs for the web monitor.
//! Parses the JSON chart format used by RPE (Re:PhiEdit).

use crate::anim::{AnimFloat, AnimVector, Keyframe};
use crate::bpm::{BpmList, Triple};
use crate::chart::{Chart, JudgeLine, JudgeLineKind, Note, NoteKind};
use crate::object::Object;
use crate::tween::{easing_from, BezierTween, TweenId, TweenMajor, TweenMinor};

use serde::Deserialize;
use std::collections::HashMap;

/// RPE chart dimensions
pub const RPE_WIDTH: f32 = 1350.0;
pub const RPE_HEIGHT: f32 = 900.0;

/// Speed ratio for height calculation
const SPEED_RATIO: f32 = 10.0 / 45.0 / 0.83175; // 10/45/HEIGHT_RATIO

/// Epsilon for float comparisons
const EPS: f32 = 1e-5;

// ============================================================================
// RPE Tween mapping (easing_type -> TweenId)
// ============================================================================

/// Maps RPE easing type to internal TweenId
#[rustfmt::skip]
pub const RPE_TWEEN_MAP: [TweenId; 30] = {
    use TweenMajor::*;
    use TweenMinor::*;
    [
        2, 2, // 0, 1: linear
        easing_from(Sine, Out), easing_from(Sine, In),       // 2, 3
        easing_from(Quad, Out), easing_from(Quad, In),       // 4, 5
        easing_from(Sine, InOut), easing_from(Quad, InOut),  // 6, 7
        easing_from(Cubic, Out), easing_from(Cubic, In),     // 8, 9
        easing_from(Quart, Out), easing_from(Quart, In),     // 10, 11
        easing_from(Cubic, InOut), easing_from(Quart, InOut),// 12, 13
        easing_from(Quint, Out), easing_from(Quint, In),     // 14, 15
        easing_from(Expo, Out), easing_from(Expo, In),       // 16, 17
        easing_from(Circ, Out), easing_from(Circ, In),       // 18, 19
        easing_from(Back, Out), easing_from(Back, In),       // 20, 21
        easing_from(Circ, InOut), easing_from(Back, InOut),  // 22, 23
        easing_from(Elastic, Out), easing_from(Elastic, In), // 24, 25
        easing_from(Bounce, Out), easing_from(Bounce, In),   // 26, 27
        easing_from(Bounce, InOut), easing_from(Elastic, InOut), // 28, 29
    ]
};

fn get_tween(easing_type: i32) -> TweenId {
    RPE_TWEEN_MAP
        .get(easing_type.max(1) as usize)
        .copied()
        .unwrap_or(RPE_TWEEN_MAP[0])
}

// ============================================================================
// RPE JSON structures
// ============================================================================

/// Triple format: [i, n, d] representing i + n/d beats
#[derive(Deserialize, Default, Clone)]
struct RpeTriple(i32, u32, u32);

impl RpeTriple {
    fn to_triple(&self) -> Triple {
        Triple::new(self.0, self.1, self.2)
    }

    fn beats(&self) -> f32 {
        self.0 as f32 + self.1 as f32 / self.2.max(1) as f32
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpeBpmItem {
    bpm: f32,
    start_time: RpeTriple,
}

fn f32_zero() -> f32 {
    0.0
}
fn f32_one() -> f32 {
    1.0
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpeEvent<T = f32> {
    #[serde(default = "f32_zero")]
    easing_left: f32,
    #[serde(default = "f32_one")]
    easing_right: f32,
    #[serde(default)]
    bezier: u8,
    #[serde(default)]
    bezier_points: [f32; 4],
    easing_type: i32,
    start: T,
    end: T,
    start_time: RpeTriple,
    end_time: RpeTriple,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpeSpeedEvent {
    start_time: RpeTriple,
    end_time: RpeTriple,
    start: f32,
    end: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpeEventLayer {
    alpha_events: Option<Vec<RpeEvent>>,
    move_x_events: Option<Vec<RpeEvent>>,
    move_y_events: Option<Vec<RpeEvent>>,
    rotate_events: Option<Vec<RpeEvent>>,
    speed_events: Option<Vec<RpeSpeedEvent>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpeExtendedEvents {
    scale_x_events: Option<Vec<RpeEvent>>,
    scale_y_events: Option<Vec<RpeEvent>>,
    incline_events: Option<Vec<RpeEvent>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpeNote {
    #[serde(rename = "type")]
    kind: u8,
    above: u8,
    start_time: RpeTriple,
    end_time: RpeTriple,
    position_x: f32,
    #[serde(default)]
    y_offset: f32,
    #[serde(default = "default_alpha")]
    alpha: u16,
    size: f32,
    speed: f32,
    #[serde(default)]
    is_fake: u8,
    #[serde(default)]
    visible_time: f32,
}

fn default_alpha() -> u16 {
    255
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpeJudgeLine {
    #[serde(rename = "Name")]
    #[allow(dead_code)]
    name: String,
    #[serde(rename = "Texture", default)]
    texture: String,
    #[serde(rename = "father")]
    parent: Option<isize>,
    event_layers: Vec<Option<RpeEventLayer>>,
    extended: Option<RpeExtendedEvents>,
    notes: Option<Vec<RpeNote>>,
    #[serde(default)]
    is_cover: u8,
    #[serde(default)]
    z_order: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpeMetadata {
    offset: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpeChart {
    #[serde(rename = "META")]
    meta: RpeMetadata,
    #[serde(rename = "BPMList")]
    bpm_list: Vec<RpeBpmItem>,
    judge_line_list: Vec<RpeJudgeLine>,
}

// ============================================================================
// Bezier curve caching
// ============================================================================

type BezierKey = (u16, i16, i16);
type BezierMap = HashMap<BezierKey, BezierTween>;

fn bezier_key<T>(event: &RpeEvent<T>) -> BezierKey {
    let p = &event.bezier_points;
    let int = |v: f32| (v * 100.0).round() as i16;
    ((int(p[0]) * 100 + int(p[1])) as u16, int(p[2]), int(p[3]))
}

fn build_bezier_map(lines: &[RpeJudgeLine]) -> BezierMap {
    let mut map = BezierMap::new();

    for line in lines {
        for layer_opt in &line.event_layers {
            if let Some(layer) = layer_opt {
                add_beziers_from_events(&mut map, layer.alpha_events.as_deref());
                add_beziers_from_events(&mut map, layer.move_x_events.as_deref());
                add_beziers_from_events(&mut map, layer.move_y_events.as_deref());
                add_beziers_from_events(&mut map, layer.rotate_events.as_deref());
            }
        }
    }

    map
}

fn add_beziers_from_events<T>(map: &mut BezierMap, events: Option<&[RpeEvent<T>]>) {
    if let Some(events) = events {
        for e in events {
            if e.bezier != 0 {
                let key = bezier_key(e);
                if !map.contains_key(&key) {
                    let p = e.bezier_points;
                    map.insert(key, BezierTween::new((p[0], p[1]), (p[2], p[3])));
                }
            }
        }
    }
}

// ============================================================================
// Event parsing
// ============================================================================

fn parse_events(
    bpm: &mut BpmList,
    events: &[RpeEvent],
    default: Option<f32>,
    _bezier_map: &BezierMap,
) -> AnimFloat {
    let mut kfs = Vec::new();

    // Add default keyframe if first event doesn't start at 0
    if let Some(def) = default {
        if !events.is_empty() && events[0].start_time.beats() != 0.0 {
            kfs.push(Keyframe::new(0.0, def, 0));
        }
    }

    for e in events {
        let start_time = bpm.time_at(&e.start_time.to_triple());
        let end_time = bpm.time_at(&e.end_time.to_triple());

        // Determine tween type
        let tween_id = get_tween(e.easing_type);

        // For bezier or clamped tweens, use the base tween
        // (simplified: we don't support bezier/clamped in keyframes yet)
        let kf_tween = if e.bezier != 0 {
            // Use bezier - store as linear for now, actual bezier handled separately
            2 // linear as placeholder
        } else if e.easing_left.abs() < EPS && (e.easing_right - 1.0).abs() < EPS {
            tween_id
        } else {
            tween_id // simplified: ignore clamping for now
        };

        kfs.push(Keyframe::new(start_time, e.start, kf_tween));
        kfs.push(Keyframe::new(end_time, e.end, 0)); // Hold after end
    }

    if kfs.is_empty() {
        AnimFloat::default()
    } else {
        AnimFloat::new(kfs)
    }
}

fn parse_events_with_factor(
    bpm: &mut BpmList,
    layers: &[Option<RpeEventLayer>],
    get_events: impl Fn(&RpeEventLayer) -> &Option<Vec<RpeEvent>>,
    factor: f32,
    _bezier_map: &BezierMap,
) -> AnimFloat {
    let mut all_events: Vec<&RpeEvent> = Vec::new();

    for layer_opt in layers {
        if let Some(layer) = layer_opt {
            if let Some(events) = get_events(layer) {
                all_events.extend(events.iter());
            }
        }
    }

    if all_events.is_empty() {
        return AnimFloat::default();
    }

    // Sort by start time
    all_events.sort_by(|a, b| {
        a.start_time
            .beats()
            .partial_cmp(&b.start_time.beats())
            .unwrap()
    });

    let mut kfs = Vec::new();
    for e in all_events {
        let start_time = bpm.time_at(&e.start_time.to_triple());
        let end_time = bpm.time_at(&e.end_time.to_triple());
        let tween_id = get_tween(e.easing_type);

        kfs.push(Keyframe::new(start_time, e.start * factor, tween_id));
        kfs.push(Keyframe::new(end_time, e.end * factor, 0));
    }

    AnimFloat::new(kfs)
}

// ============================================================================
// Speed events -> Height animation
// ============================================================================

fn parse_speed_events(
    bpm: &mut BpmList,
    layers: &[Option<RpeEventLayer>],
    max_time: f32,
) -> AnimFloat {
    let mut all_speed_events: Vec<&RpeSpeedEvent> = Vec::new();

    for layer_opt in layers {
        if let Some(layer) = layer_opt {
            if let Some(events) = &layer.speed_events {
                all_speed_events.extend(events.iter());
            }
        }
    }

    if all_speed_events.is_empty() {
        return AnimFloat::default();
    }

    // Build speed keyframes
    let mut speed_kfs = Vec::new();
    for e in &all_speed_events {
        let start_time = bpm.time_at(&e.start_time.to_triple());
        let end_time = bpm.time_at(&e.end_time.to_triple());
        speed_kfs.push(Keyframe::new(start_time, e.start * SPEED_RATIO, 2)); // Linear
        speed_kfs.push(Keyframe::new(end_time, e.end * SPEED_RATIO, 0));
    }

    if speed_kfs.is_empty() {
        return AnimFloat::default();
    }

    let speed_anim = AnimFloat::new(speed_kfs);

    // Integrate speed to get height (simplified: linear interpolation)
    let mut height_kfs = Vec::new();
    let mut height = 0.0_f32;
    let mut prev_time = 0.0_f32;

    // Sample at key points
    let mut times: Vec<f32> = all_speed_events
        .iter()
        .flat_map(|e| {
            vec![
                bpm.time_at(&e.start_time.to_triple()),
                bpm.time_at(&e.end_time.to_triple()),
            ]
        })
        .collect();
    times.push(max_time);
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    times.dedup();

    let mut speed_anim_mut = speed_anim;
    for &t in &times {
        if t > prev_time {
            speed_anim_mut.set_time(prev_time);
            let speed_start = speed_anim_mut.now();
            speed_anim_mut.set_time(t - 0.0001);
            let speed_end = speed_anim_mut.now();

            height_kfs.push(Keyframe::new(prev_time, height, 2));
            height += (speed_start + speed_end) * (t - prev_time) / 2.0;
        }
        prev_time = t;
    }
    height_kfs.push(Keyframe::new(max_time, height, 0));

    AnimFloat::new(height_kfs)
}

// ============================================================================
// Note parsing
// ============================================================================

fn parse_notes(
    bpm: &mut BpmList,
    rpe_notes: Vec<RpeNote>,
    height_anim: &mut AnimFloat,
) -> Vec<Note> {
    let mut notes = Vec::new();

    for rpe_note in rpe_notes {
        let time = bpm.time_at(&rpe_note.start_time.to_triple());
        height_anim.set_time(time);
        let note_height = height_anim.now();

        let y_offset = rpe_note.y_offset * 2.0 / RPE_HEIGHT * rpe_note.speed;

        let kind = match rpe_note.kind {
            1 => NoteKind::Click,
            2 => {
                let end_time = bpm.time_at(&rpe_note.end_time.to_triple());
                height_anim.set_time(end_time);
                NoteKind::Hold {
                    end_time,
                    end_height: height_anim.now(),
                }
            }
            3 => NoteKind::Flick,
            4 => NoteKind::Drag,
            _ => NoteKind::Click, // fallback
        };

        // Build note object with animations
        let alpha = if rpe_note.visible_time >= time {
            if rpe_note.alpha >= 255 {
                AnimFloat::default()
            } else {
                AnimFloat::fixed(rpe_note.alpha as f32 / 255.0)
            }
        } else {
            let alpha_val = (rpe_note.alpha.min(255) as f32) / 255.0;
            AnimFloat::new(vec![
                Keyframe::new(0.0, 0.0, 0),
                Keyframe::new(time - rpe_note.visible_time, alpha_val, 0),
            ])
        };

        let translation = AnimVector::fixed(rpe_note.position_x / (RPE_WIDTH / 2.0), y_offset);

        let scale = AnimVector::fixed(rpe_note.size, rpe_note.size);

        let mut note = Note::new(kind, time, note_height);
        note.object = Object {
            alpha,
            translation,
            scale,
            ..Default::default()
        };
        note.speed = rpe_note.speed;
        note.above = rpe_note.above == 1;
        note.fake = rpe_note.is_fake != 0;

        notes.push(note);
    }

    // Sort by time
    notes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());

    // Mark multiple hints (notes at same time)
    let mut i = 0;
    while i < notes.len() {
        let time = notes[i].time;
        let mut j = i + 1;
        while j < notes.len() && (notes[j].time - time).abs() < 0.001 {
            j += 1;
        }
        if j - i > 1 {
            for k in i..j {
                notes[k].multiple_hint = true;
            }
        }
        i = j;
    }

    notes
}

// ============================================================================
// Judge line parsing
// ============================================================================

fn parse_judge_line(
    bpm: &mut BpmList,
    rpe: RpeJudgeLine,
    max_time: f32,
    bezier_map: &BezierMap,
) -> JudgeLine {
    let mut line = JudgeLine::default();

    // Parse basic properties
    line.z_order = rpe.z_order;
    line.parent = rpe
        .parent
        .and_then(|p| if p >= 0 { Some(p as usize) } else { None });
    line.show_below = rpe.is_cover == 0;

    // Texture
    if !rpe.texture.is_empty() && rpe.texture != "line.png" {
        line.kind = JudgeLineKind::Texture(rpe.texture);
    }

    // Parse event layers
    let layers = &rpe.event_layers;

    // Alpha
    line.alpha =
        parse_events_with_factor(bpm, layers, |l| &l.alpha_events, 1.0 / 255.0, bezier_map);

    // Position (move_x, move_y)
    let move_x = parse_events_with_factor(
        bpm,
        layers,
        |l| &l.move_x_events,
        2.0 / RPE_WIDTH,
        bezier_map,
    );
    let move_y = parse_events_with_factor(
        bpm,
        layers,
        |l| &l.move_y_events,
        2.0 / RPE_HEIGHT,
        bezier_map,
    );
    line.object.translation = AnimVector::new(move_x, move_y);

    // Rotation
    line.object.rotation =
        parse_events_with_factor(bpm, layers, |l| &l.rotate_events, -1.0, bezier_map);

    // Speed -> Height
    line.height = parse_speed_events(bpm, layers, max_time);

    // Extended events
    if let Some(ext) = &rpe.extended {
        if let Some(events) = &ext.scale_x_events {
            line.object.scale.x = parse_events(bpm, events, Some(1.0), bezier_map);
        }
        if let Some(events) = &ext.scale_y_events {
            line.object.scale.y = parse_events(bpm, events, Some(1.0), bezier_map);
        }
        if let Some(events) = &ext.incline_events {
            line.incline = parse_events(bpm, events, Some(0.0), bezier_map);
        }
    }

    // Parse notes
    if let Some(notes) = rpe.notes {
        let mut height_anim = line.height.clone();
        line.notes = parse_notes(bpm, notes, &mut height_anim);
    }

    line
}

// ============================================================================
// Main parser
// ============================================================================

/// Parse error type
#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parse error: {}", self.0)
    }
}

impl std::error::Error for ParseError {}

/// Parse an RPE chart from JSON string
pub fn parse_rpe(json: &str) -> Result<Chart, ParseError> {
    let rpe: RpeChart =
        serde_json::from_str(json).map_err(|e| ParseError(format!("JSON parse error: {}", e)))?;

    // Build BPM list
    let bpm_ranges: Vec<(f32, f32)> = rpe
        .bpm_list
        .iter()
        .map(|item| (item.start_time.beats(), item.bpm))
        .collect();
    let mut bpm_list = BpmList::new(bpm_ranges);

    // Calculate max time
    let max_time = calculate_max_time(&rpe, &mut bpm_list);

    // Build bezier map
    let bezier_map = build_bezier_map(&rpe.judge_line_list);

    // Parse judge lines
    let mut lines = Vec::new();
    for rpe_line in rpe.judge_line_list {
        bpm_list.reset();
        let line = parse_judge_line(&mut bpm_list, rpe_line, max_time, &bezier_map);
        lines.push(line);
    }

    // Offset in seconds
    let offset = rpe.meta.offset as f32 / 1000.0;

    bpm_list.reset();
    Ok(Chart::new(offset, lines, bpm_list))
}

fn calculate_max_time(rpe: &RpeChart, bpm: &mut BpmList) -> f32 {
    let mut max_time = 0.0_f32;

    for line in &rpe.judge_line_list {
        if let Some(notes) = &line.notes {
            for note in notes {
                let t = bpm.time_at(&note.end_time.to_triple());
                max_time = max_time.max(t);
            }
        }
    }

    max_time + 1.0 // Add buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tween_map() {
        assert_eq!(RPE_TWEEN_MAP[0], 2); // Linear
        assert_eq!(RPE_TWEEN_MAP[1], 2); // Linear
    }

    #[test]
    fn test_rpe_triple() {
        let t = RpeTriple(1, 1, 2);
        assert!((t.beats() - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_parse_empty_chart() {
        let json = r#"{
            "META": { "offset": 0 },
            "BPMList": [{ "bpm": 120, "startTime": [0, 0, 1] }],
            "judgeLineList": []
        }"#;

        let chart = parse_rpe(json).unwrap();
        assert_eq!(chart.line_count(), 0);
    }
}
