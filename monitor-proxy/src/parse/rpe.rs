//! RPE chart format parser
//!
//! Ported from prpr/src/parse/rpe.rs for the web monitor.
//! Parses the JSON chart format used by RPE (Re:PhiEdit).

use super::{process_lines, ResourceLoader, RPE_TWEEN_MAP};
use monitor_common::core::{
    colors::WHITE, Anim, AnimFloat, AnimVector, AudioClip, BezierTween, BpmList, Chart, Color,
    CtrlObject, GifFrames, HitSound, HitSoundMap, JudgeLine, JudgeLineKind, Keyframe, Note,
    NoteKind, Object, Texture, Triple, Tweenable, UIElement, EPS, HEIGHT_RATIO,
};

use anyhow::{bail, Context, Result};
use image::{codecs::gif, AnimationDecoder, DynamicImage};
use serde::Deserialize;
use std::{collections::HashMap, io::Cursor, time::Duration};

pub const RPE_WIDTH: f32 = 1350.;
pub const RPE_HEIGHT: f32 = 900.;
const SPEED_RATIO: f32 = 10. / 45. / HEIGHT_RATIO;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEBpmItem {
    bpm: f32,
    start_time: Triple,
}

fn f32_zero() -> f32 {
    0.
}

fn f32_one() -> f32 {
    1.
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEEvent<T = f32> {
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
    start_time: Triple,
    end_time: Triple,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPECtrlEvent {
    easing: u8,
    x: f32,
    #[serde(flatten)]
    value: HashMap<String, f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPESpeedEvent {
    start_time: Triple,
    end_time: Triple,
    start: f32,
    end: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEEventLayer {
    alpha_events: Option<Vec<RPEEvent>>,
    move_x_events: Option<Vec<RPEEvent>>,
    move_y_events: Option<Vec<RPEEvent>>,
    rotate_events: Option<Vec<RPEEvent>>,
    speed_events: Option<Vec<RPESpeedEvent>>,
}

#[derive(Clone, Deserialize)]
struct RGBColor(u8, u8, u8);
impl From<RGBColor> for Color {
    fn from(RGBColor(r, g, b): RGBColor) -> Self {
        Self::from_rgba(r, g, b, 255)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEExtendedEvents {
    color_events: Option<Vec<RPEEvent<RGBColor>>>,
    text_events: Option<Vec<RPEEvent<String>>>,
    scale_x_events: Option<Vec<RPEEvent>>,
    scale_y_events: Option<Vec<RPEEvent>>,
    incline_events: Option<Vec<RPEEvent>>,
    paint_events: Option<Vec<RPEEvent>>,
    gif_events: Option<Vec<RPEEvent>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPENote {
    #[serde(rename = "type")]
    kind: u8,
    above: u8,
    start_time: Triple,
    end_time: Triple,
    position_x: f32,
    y_offset: f32,
    alpha: u16,
    hitsound: Option<String>,
    size: f32,
    speed: f32,
    is_fake: u8,
    visible_time: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEJudgeLine {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Texture")]
    texture: String,
    #[serde(rename = "father")]
    parent: Option<isize>,
    event_layers: Vec<Option<RPEEventLayer>>,
    extended: Option<RPEExtendedEvents>,
    notes: Option<Vec<RPENote>>,
    is_cover: u8,
    #[serde(default)]
    z_order: i32,
    #[serde(rename = "attachUI")]
    attach_ui: Option<UIElement>,

    #[serde(default)]
    pos_control: Vec<RPECtrlEvent>,
    #[serde(default)]
    size_control: Vec<RPECtrlEvent>,
    #[serde(default)]
    alpha_control: Vec<RPECtrlEvent>,
    #[serde(default)]
    y_control: Vec<RPECtrlEvent>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEMetadata {
    offset: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEChart {
    #[serde(rename = "META")]
    meta: RPEMetadata,
    #[serde(rename = "BPMList")]
    bpm_list: Vec<RPEBpmItem>,
    judge_line_list: Vec<RPEJudgeLine>,
}

type BezierMap = HashMap<(u16, i16, i16), BezierTween>;

fn bezier_key<T>(event: &RPEEvent<T>) -> (u16, i16, i16) {
    let p = &event.bezier_points;
    let int = |p: f32| (p * 100.).round() as i16;
    ((int(p[0]) * 100 + int(p[1])) as u16, int(p[2]), int(p[3]))
}

fn parse_events<T: Tweenable, V: Clone + Into<T>>(
    r: &mut BpmList,
    rpe: &[RPEEvent<V>],
    default: Option<T>,
    _bezier_map: &BezierMap,
) -> Result<Anim<T>> {
    let mut kfs = Vec::new();
    if let Some(default) = default {
        if rpe.get(0).map_or(false, |e| e.start_time.beats() != 0.0) {
            kfs.push(Keyframe::new(0.0, default, 0));
        }
    }
    for e in rpe {
        let time = r.time_at(&e.start_time);
        let value = e.start.clone().into();

        if e.bezier != 0 {
            kfs.push(Keyframe::with_bezier(
                time,
                value,
                (e.bezier_points[0], e.bezier_points[1]),
                (e.bezier_points[2], e.bezier_points[3]),
            ));
        } else {
            let tween = RPE_TWEEN_MAP
                .get(e.easing_type.max(1) as usize)
                .copied()
                .unwrap_or(RPE_TWEEN_MAP[0]);
            if e.easing_left.abs() < EPS && (e.easing_right - 1.0).abs() < EPS {
                kfs.push(Keyframe::new(time, value, tween));
            } else {
                kfs.push(Keyframe::with_clamped(
                    time,
                    value,
                    e.easing_left..e.easing_right,
                    tween,
                ));
            }
        }

        kfs.push(Keyframe::new(
            r.time_at(&e.end_time),
            e.end.clone().into(),
            0,
        ));
    }
    Ok(Anim::new(kfs))
}

fn parse_speed_events(r: &mut BpmList, rpe: &[RPEEventLayer], max_time: f32) -> Result<AnimFloat> {
    let rpe_events: Vec<_> = rpe
        .iter()
        .filter_map(|it| it.speed_events.as_ref())
        .collect();
    if rpe_events.is_empty() {
        return Ok(AnimFloat::default());
    };
    let anis: Vec<_> = rpe_events
        .into_iter()
        .map(|it| {
            let mut kfs = Vec::new();
            for e in it {
                kfs.push(Keyframe::new(r.time_at(&e.start_time), e.start, 2));
                kfs.push(Keyframe::new(r.time_at(&e.end_time), e.end, 0));
            }
            AnimFloat::new(kfs)
        })
        .collect();
    let mut pts: Vec<_> = anis
        .iter()
        .flat_map(|it| it.keyframes.iter().map(|it| it.time))
        .collect();
    pts.push(max_time);
    pts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    pts.dedup();
    let mut sani = AnimFloat::chain(anis);
    sani.map_value(|v| v * SPEED_RATIO);
    for i in 0..(pts.len() - 1) {
        let now_time = pts[i];
        let end_time = pts[i + 1];
        sani.set_time(now_time);
        let speed = sani.now();
        sani.set_time(end_time - 1e-4);
        let end_speed = sani.now();
        if speed.signum() * end_speed.signum() < 0. {
            if (speed - end_speed).abs() > EPS {
                let t = f32::tween(&now_time, &end_time, speed / (speed - end_speed));
                pts.push(t);
            }
        }
    }
    pts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    pts.dedup();
    let mut kfs = Vec::new();
    let mut height = 0.0;
    for i in 0..(pts.len() - 1) {
        let now_time = pts[i];
        let end_time = pts[i + 1];
        sani.set_time(now_time);
        let speed = sani.now();
        sani.set_time(end_time - 1e-4);
        let end_speed = sani.now();
        kfs.push(if (speed - end_speed).abs() < EPS {
            Keyframe::new(now_time, height, 2)
        } else if speed.abs() > end_speed.abs() {
            Keyframe::with_clamped(
                now_time,
                height,
                0.0..(1. - end_speed / speed),
                7, // QuadOut
            )
        } else {
            Keyframe::with_clamped(
                now_time,
                height,
                (speed / end_speed)..1.,
                6, // QuadIn
            )
        });
        height += (speed + end_speed) * (end_time - now_time) / 2.;
    }
    kfs.push(Keyframe::new(max_time, height, 0));
    Ok(AnimFloat::new(kfs))
}

fn parse_gif_events<V: Clone + Into<f32>>(
    r: &mut BpmList,
    rpe: &[RPEEvent<V>],
    _bezier_map: &BezierMap,
    gif: &GifFrames,
) -> Result<Anim<f32>> {
    let mut kfs = Vec::new();
    kfs.push(Keyframe::new(0.0, 0.0, 2));
    let mut next_rep_time: u128 = 0;
    for e in rpe {
        while r.time_at(&e.start_time) > next_rep_time as f32 / 1000. {
            kfs.push(Keyframe::new(next_rep_time as f32 / 1000., 1.0, 0));
            kfs.push(Keyframe::new(next_rep_time as f32 / 1000., 0.0, 2));
            next_rep_time += gif.total_time;
        }
        let stop_prog =
            1. - (next_rep_time as f32 - r.time_at(&e.start_time) * 1000.) / gif.total_time as f32;
        kfs.push(Keyframe::new(r.time_at(&e.start_time), stop_prog, 0));

        let time = r.time_at(&e.start_time);
        let value = e.start.clone().into();

        if e.bezier != 0 {
            kfs.push(Keyframe::with_bezier(
                time,
                value,
                (e.bezier_points[0], e.bezier_points[1]),
                (e.bezier_points[2], e.bezier_points[3]),
            ));
        } else {
            let tween = RPE_TWEEN_MAP
                .get(e.easing_type.max(1) as usize)
                .copied()
                .unwrap_or(RPE_TWEEN_MAP[0]);
            if e.easing_left.abs() < EPS && (e.easing_right - 1.0).abs() < EPS {
                kfs.push(Keyframe::new(time, value, tween));
            } else {
                kfs.push(Keyframe::with_clamped(
                    time,
                    value,
                    e.easing_left..e.easing_right,
                    tween,
                ));
            }
        }

        kfs.push(Keyframe::new(
            r.time_at(&e.end_time),
            e.end.clone().into(),
            2, // Linear
        ));
        next_rep_time = (r.time_at(&e.end_time) * 1000.
            + gif.total_time as f32 * (1. - Into::<f32>::into(e.end.clone())))
        .round() as u128;
    }

    const GIF_MAX_TIME: f32 = 2000.;
    while GIF_MAX_TIME > next_rep_time as f32 / 1000. {
        kfs.push(Keyframe::new(next_rep_time as f32 / 1000., 1.0, 0));
        kfs.push(Keyframe::new(next_rep_time as f32 / 1000., 0.0, 2));
        next_rep_time += gif.total_time;
    }
    Ok(Anim::new(kfs))
}

fn get_default_hitsound(kind: &NoteKind) -> HitSound {
    match kind {
        NoteKind::Click | NoteKind::Hold { .. } => HitSound::Click,
        NoteKind::Flick => HitSound::Flick,
        NoteKind::Drag => HitSound::Drag,
    }
}

async fn parse_notes(
    r: &mut BpmList,
    rpe: Vec<RPENote>,
    height: &mut AnimFloat,
    fs: &mut dyn ResourceLoader,
    hitsounds: &mut HitSoundMap,
) -> Result<Vec<Note>> {
    let mut notes = Vec::new();
    for note in rpe {
        let time: f32 = r.time_at(&note.start_time);
        height.set_time(time);
        let note_height = height.now();
        let y_offset = note.y_offset * 2. / RPE_HEIGHT * note.speed;
        let kind = match note.kind {
            1 => NoteKind::Click,
            2 => {
                let end_time = r.time_at(&note.end_time);
                height.set_time(end_time);
                NoteKind::Hold {
                    end_time,
                    end_height: height.now(),
                }
            }
            3 => NoteKind::Flick,
            4 => NoteKind::Drag,
            _ => bail!("unknown-note-type: {}", note.kind),
        };

        let hitsound = match &note.hitsound {
            Some(s) if s == "flick.mp3" => Some(HitSound::Flick),
            Some(s) if s == "tap.mp3" => Some(HitSound::Click),
            Some(s) if s == "drag.mp3" => Some(HitSound::Drag),
            Some(s) => {
                let hit_sound = HitSound::Custom(s.clone());
                if !hitsounds.contains_key(&hit_sound) {
                    let data = fs
                        .load_file(s)
                        .await
                        .with_context(|| format!("hitsound-load-failed: {}", s))?;

                    let ext = std::path::Path::new(s)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("mp3");
                    let temp_path = std::env::temp_dir().join(format!(
                        "phira_hit_{}.{}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_nanos(),
                        ext
                    ));
                    std::fs::write(&temp_path, &data)?;
                    let clip = AudioClip::load_from_path(&temp_path)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;
                    let _ = std::fs::remove_file(&temp_path);
                    hitsounds.insert(hit_sound.clone(), clip);
                }
                Some(hit_sound)
            }
            None => None,
        };

        let hitsound = hitsound.or_else(|| Some(get_default_hitsound(&kind)));
        notes.push(Note {
            object: Object {
                alpha: if note.visible_time >= time {
                    if note.alpha >= 255 {
                        AnimFloat::default()
                    } else {
                        AnimFloat::fixed(note.alpha as f32 / 255.)
                    }
                } else {
                    let alpha = note.alpha.min(255) as f32 / 255.;
                    AnimFloat::new(vec![
                        Keyframe::new(0.0, 0.0, 0),
                        Keyframe::new(time - note.visible_time, alpha, 0),
                    ])
                },
                translation: AnimVector::new(
                    AnimFloat::fixed(note.position_x / (RPE_WIDTH / 2.)),
                    AnimFloat::fixed(y_offset),
                ),
                scale: AnimVector::new(AnimFloat::fixed(note.size), AnimFloat::fixed(note.size)),
                ..Default::default()
            },
            kind,
            time,
            height: note_height,
            speed: note.speed,
            above: note.above == 1,
            multiple_hint: false,
            fake: note.is_fake != 0,
            hitsound,
        })
    }
    Ok(notes)
}

fn parse_ctrl_events(rpe: &[RPECtrlEvent], key: &str) -> AnimFloat {
    let vals: Vec<_> = rpe.iter().map(|it| it.value[key]).collect();
    if rpe.is_empty() || (rpe.len() == 2 && rpe[0].easing == 1 && (vals[0] - 1.).abs() < 1e-4) {
        return AnimFloat::default();
    }
    AnimFloat::new(
        rpe.iter()
            .zip(vals.into_iter())
            .map(|(it, val)| {
                Keyframe::new(
                    it.x,
                    val,
                    RPE_TWEEN_MAP
                        .get(it.easing.max(1) as usize)
                        .copied()
                        .unwrap_or(RPE_TWEEN_MAP[0]),
                )
            })
            .collect(),
    )
}

async fn parse_judge_line(
    r: &mut BpmList,
    rpe: RPEJudgeLine,
    max_time: f32,
    fs: &mut dyn ResourceLoader,
    bezier_map: &BezierMap,
    line_texture_map: &mut HashMap<String, Texture>,
    hitsounds: &mut HitSoundMap,
) -> Result<JudgeLine> {
    let event_layers: Vec<_> = rpe.event_layers.into_iter().flatten().collect();
    fn events_with_factor(
        r: &mut BpmList,
        event_layers: &[RPEEventLayer],
        get: impl Fn(&RPEEventLayer) -> &Option<Vec<RPEEvent>>,
        factor: f32,
        desc: &str,
        bezier_map: &BezierMap,
    ) -> Result<AnimFloat> {
        let anis: Vec<_> = event_layers
            .iter()
            .filter_map(|it| {
                get(it)
                    .as_ref()
                    .map(|es| parse_events(r, es, None, bezier_map))
            })
            .collect::<Result<_>>()
            .with_context(|| format!("type-events-parse-failed: {}", desc))?;
        let mut res = AnimFloat::chain(anis);
        res.map_value(|v| v * factor);
        Ok(res)
    }
    let mut height = parse_speed_events(r, &event_layers, max_time)?;
    let notes = parse_notes(r, rpe.notes.unwrap_or_default(), &mut height, fs, hitsounds).await?;

    Ok(JudgeLine {
        object: Object {
            alpha: events_with_factor(
                r,
                &event_layers,
                |it| &it.alpha_events,
                1. / 255.,
                "alpha",
                bezier_map,
            )?,
            rotation: events_with_factor(
                r,
                &event_layers,
                |it| &it.rotate_events,
                -1.,
                "rotate",
                bezier_map,
            )?,
            translation: AnimVector::new(
                events_with_factor(
                    r,
                    &event_layers,
                    |it| &it.move_x_events,
                    2. / RPE_WIDTH,
                    "move X",
                    bezier_map,
                )?,
                events_with_factor(
                    r,
                    &event_layers,
                    |it| &it.move_y_events,
                    2. / RPE_HEIGHT,
                    "move Y",
                    bezier_map,
                )?,
            ),
            scale: {
                fn parse(
                    r: &mut BpmList,
                    opt: &Option<Vec<RPEEvent>>,
                    factor: f32,
                    bezier_map: &BezierMap,
                ) -> Result<AnimFloat> {
                    let mut res = opt
                        .as_ref()
                        .map(|it| parse_events(r, it, None, bezier_map))
                        .transpose()?
                        .unwrap_or_default();
                    res.map_value(|v| v * factor);
                    Ok(res)
                }
                let factor = if rpe.texture == "line.png" {
                    1.
                } else {
                    2. / RPE_WIDTH
                };
                rpe.extended
                    .as_ref()
                    .map(|e| -> Result<_> {
                        Ok(AnimVector::new(
                            parse(
                                r,
                                &e.scale_x_events,
                                factor
                                    * if rpe.texture == "line.png"
                                        && rpe.extended.as_ref().map_or(true, |it| {
                                            it.text_events.as_ref().map_or(true, |it| it.is_empty())
                                        })
                                        && rpe.attach_ui.is_none()
                                    {
                                        0.5
                                    } else {
                                        1.
                                    },
                                bezier_map,
                            )?,
                            parse(r, &e.scale_y_events, factor, bezier_map)?,
                        ))
                    })
                    .transpose()?
                    .unwrap_or_default()
            },
        },
        ctrl_obj: CtrlObject {
            alpha: parse_ctrl_events(&rpe.alpha_control, "alpha"),
            size: parse_ctrl_events(&rpe.size_control, "size"),
            pos: parse_ctrl_events(&rpe.pos_control, "pos"),
            y: parse_ctrl_events(&rpe.y_control, "y"),
        },
        height,
        incline: if let Some(events) = rpe
            .extended
            .as_ref()
            .and_then(|e| e.incline_events.as_ref())
        {
            parse_events(r, events, Some(0.), bezier_map).context("incline-events-parse-failed")?
        } else {
            AnimFloat::default()
        },
        notes,
        kind: if rpe.texture == "line.png" {
            if let Some(events) = rpe.extended.as_ref().and_then(|e| e.paint_events.as_ref()) {
                JudgeLineKind::Paint(
                    parse_events(r, events, Some(-1.), bezier_map)
                        .context("paint-events-parse-failed")?,
                )
            } else if let Some(extended) = rpe.extended.as_ref() {
                if let Some(events) = extended.text_events.as_ref() {
                    JudgeLineKind::Text(
                        parse_events(r, events, Some(String::new()), bezier_map)
                            .context("text-events-parse-failed")?,
                    )
                } else {
                    JudgeLineKind::Normal
                }
            } else {
                JudgeLineKind::Normal
            }
        } else if let Some(extended) = rpe.extended.as_ref() {
            if let Some(events) = extended.gif_events.as_ref() {
                let data = fs
                    .load_file(&rpe.texture)
                    .await
                    .with_context(|| format!("gif-load-failed: {}", rpe.texture))?;

                let decoder = gif::GifDecoder::new(Cursor::new(data))?;
                let frames_vec: Vec<_> = decoder.into_frames().collect_frames()?;

                let frames_list: Vec<(u128, Texture)> = frames_vec
                    .into_iter()
                    .map(|frame| {
                        let delay: Duration = frame.delay().into();
                        let img = DynamicImage::ImageRgba8(frame.into_buffer());
                        (delay.as_millis(), Texture::new(img))
                    })
                    .collect();

                let total_time = frames_list.iter().map(|(d, _)| *d).sum();
                let frames = GifFrames {
                    frames: frames_list,
                    total_time,
                };

                let events = parse_gif_events(r, events, bezier_map, &frames)
                    .context("gif-events-parse-failed")?;
                JudgeLineKind::TextureGif(events, frames, rpe.texture.clone())
            } else {
                if let Some(texture) = line_texture_map.get(&rpe.texture) {
                    JudgeLineKind::Texture(texture.clone(), rpe.texture.clone())
                } else {
                    let data = fs
                        .load_file(&rpe.texture)
                        .await
                        .with_context(|| format!("illustration-load-failed: {}", rpe.texture))?;
                    let texture = Texture::new(image::load_from_memory(&data)?);
                    line_texture_map.insert(rpe.texture.clone(), texture.clone());
                    JudgeLineKind::Texture(texture, rpe.texture.clone())
                }
            }
        } else {
            if let Some(texture) = line_texture_map.get(&rpe.texture) {
                JudgeLineKind::Texture(texture.clone(), rpe.texture.clone())
            } else {
                let data = fs
                    .load_file(&rpe.texture)
                    .await
                    .with_context(|| format!("illustration-load-failed: {}", rpe.texture))?;
                let texture = Texture::new(image::load_from_memory(&data)?);
                line_texture_map.insert(rpe.texture.clone(), texture.clone());
                JudgeLineKind::Texture(texture, rpe.texture.clone())
            }
        },
        color: if let Some(events) = rpe.extended.as_ref().and_then(|e| e.color_events.as_ref()) {
            parse_events(r, events, Some(WHITE), bezier_map).context("color-events-parse-failed")?
        } else {
            Anim::default()
        },
        parent: {
            let parent = rpe.parent.unwrap_or(-1);
            if parent == -1 {
                None
            } else {
                Some(parent as usize)
            }
        },
        z_index: rpe.z_order,
        show_below: rpe.is_cover != 1,
        attach_ui: rpe.attach_ui,
    })
}

fn add_bezier<T>(map: &mut BezierMap, event: &RPEEvent<T>) {
    if event.bezier != 0 {
        let p = &event.bezier_points;
        let int = |p: f32| (p * 100.).round() as i16;
        map.entry(((int(p[0]) * 100 + int(p[1])) as u16, int(p[2]), int(p[3])))
            .or_insert_with(|| BezierTween::new((p[0], p[1]), (p[2], p[3])));
    }
}

macro_rules! process_bezier {
    ($event_layer:expr, $map:expr, $($field:ident),*) => {
        $(
            if let Some(events) = &$event_layer.$field {
                for event in events {
                     add_bezier($map, event);
                }
            }
        )*
    };
}

fn get_bezier_map(rpe: &RPEChart) -> BezierMap {
    let mut map = HashMap::new();
    for line in &rpe.judge_line_list {
        for event_layer in line.event_layers.iter().flatten() {
            process_bezier!(
                event_layer,
                &mut map,
                alpha_events,
                move_x_events,
                move_y_events,
                rotate_events
            );
        }
        if let Some(ext_layer) = &line.extended {
            process_bezier!(
                ext_layer,
                &mut map,
                paint_events,
                scale_x_events,
                scale_y_events,
                gif_events,
                incline_events,
                text_events,
                color_events
            );
        }
    }
    map
}

pub async fn parse_rpe(source: &str, fs: &mut dyn ResourceLoader) -> Result<Chart> {
    let rpe: RPEChart = serde_json::from_str(source).context("json-parse-failed")?;
    let bezier_map = get_bezier_map(&rpe);
    let mut r = BpmList::new(
        rpe.bpm_list
            .iter()
            .map(|it| (it.start_time.beats(), it.bpm))
            .collect(),
    );
    fn vec<'a, T>(v: &'a Option<Vec<T>>) -> impl Iterator<Item = &'a T> {
        v.iter().flat_map(|it| it.iter())
    }

    let max_time = rpe
        .judge_line_list
        .iter()
        .map(|line| {
            line.notes
                .as_ref()
                .map(|notes| {
                    notes
                        .iter()
                        .map(|note| r.time_at(&note.end_time))
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap_or_default()
                })
                .unwrap_or_default()
                .max(
                    line.event_layers
                        .iter()
                        .filter_map(|it| {
                            it.as_ref().map(|layer| {
                                vec(&layer.alpha_events)
                                    .chain(vec(&layer.move_x_events))
                                    .chain(vec(&layer.move_y_events))
                                    .chain(vec(&layer.rotate_events))
                                    .map(|it| r.time_at(&it.end_time))
                                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                                    .unwrap_or_default()
                            })
                        })
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap_or_default(),
                )
                .max(
                    line.extended
                        .as_ref()
                        .map(|e| {
                            vec(&e.scale_x_events)
                                .chain(vec(&e.scale_y_events))
                                .map(|it| r.time_at(&it.end_time))
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or_default()
                                .max(
                                    vec(&e.text_events)
                                        .map(|it| r.time_at(&it.end_time))
                                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                                        .unwrap_or_default(),
                                )
                        })
                        .unwrap_or_default(),
                )
        })
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or_default()
        + 1.;

    let mut lines = Vec::new();
    let mut line_texture_map = HashMap::new();
    let mut hitsounds = HashMap::new();
    for (id, rpe_line) in rpe.judge_line_list.into_iter().enumerate() {
        let name = rpe_line.name.clone();
        lines.push(
            parse_judge_line(
                &mut r,
                rpe_line,
                max_time,
                fs,
                &bezier_map,
                &mut line_texture_map,
                &mut hitsounds,
            )
            .await
            .with_context(|| format!("judge-line-location-name: {} {}", id, name))?,
        );
    }

    fn has_cycle(line: &JudgeLine, lines: &[JudgeLine], visited: &mut Vec<usize>) -> Option<usize> {
        if let Some(parent_index) = line.parent {
            if visited.contains(&parent_index) {
                return Some(parent_index);
            }
            visited.push(parent_index);
            if parent_index < lines.len() {
                return has_cycle(&lines[parent_index], lines, visited);
            }
        }
        None
    }
    for (i, line) in lines.iter().enumerate() {
        let mut vec = Vec::new();
        vec.push(i);
        if let Some(l) = has_cycle(line, &lines, &mut vec) {
            bail!("found infinite recursive parent relations: {}", l)
        }
    }

    process_lines(&mut lines);
    let mut chart = Chart::new(rpe.meta.offset as f32 / 1000.0, lines, r);
    chart.hitsounds = hitsounds;
    Ok(chart)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::{future::Future, pin::Pin};

    struct MockLoader;
    impl ResourceLoader for MockLoader {
        fn load_file<'a>(
            &'a mut self,
            _path: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
            Box::pin(async {
                // 1x1 transparent PNG
                let png = vec![
                    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49,
                    0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06,
                    0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44,
                    0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d,
                    0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42,
                    0x60, 0x82,
                ];
                Ok(png)
            })
        }
    }

    #[tokio::test]
    async fn test_parse_real_chart() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // Need to go up to workspace root then to phira
        // monitor-common is in monitor-common/
        // phira is in ../phira/
        // So ../../phira/ should work if we are in monitor-common/src/parse/
        // But CARGO_MANIFEST_DIR is .../monitor-common
        let chart_path = manifest_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("phira/target/release/data/charts/download/36238/4763292798521731.json");

        println!("Loading chart from: {:?}", chart_path);
        let json = std::fs::read_to_string(&chart_path).expect("Failed to read chart file");
        let mut loader = MockLoader;

        // Remove BOM if present (some JSON files have it)
        let json = json.trim_start_matches('\u{feff}');

        let result = parse_rpe(json, &mut loader).await;

        match &result {
            Ok(chart) => {
                println!("Successfully parsed chart!");
                println!("JudgeLines: {}", chart.lines.len());
                println!("Offset: {}", chart.offset);
                assert!(chart.lines.len() > 0);
            }
            Err(e) => {
                panic!("Failed to parse chart: {:?}", e);
            }
        }
    }
}
