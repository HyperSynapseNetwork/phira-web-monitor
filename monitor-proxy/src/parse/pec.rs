use super::{process_lines, RPE_TWEEN_MAP};
use monitor_common::core::{
    Anim, AnimFloat, AnimVector, BpmList, Chart, JudgeLine, JudgeLineKind, Keyframe, Note,
    NoteKind, Object, TweenId, EPS,
};
use ordered_float::{Float, NotNan};

pub trait NotNanExt: Sized {
    fn not_nan(self) -> NotNan<Self>;
}

impl<T: Sized + Float> NotNanExt for T {
    fn not_nan(self) -> NotNan<Self> {
        NotNan::new(self).unwrap()
    }
}

use anyhow::{anyhow, bail, Context, Result};

trait Take {
    fn take_f32(&mut self) -> Result<f32>;
    fn take_usize(&mut self) -> Result<usize>;
    fn take_tween(&mut self) -> Result<TweenId>;
    fn take_time(&mut self, b: &mut BpmList) -> Result<f32>;
}

impl<'a, T: Iterator<Item = &'a str>> Take for T {
    fn take_f32(&mut self) -> Result<f32> {
        self.next()
            .ok_or_else(|| anyhow!("unexpected eol"))
            .and_then(|it| it.parse::<f32>().map_err(|e| anyhow!(e)))
            .context("expected f32")
    }

    fn take_usize(&mut self) -> Result<usize> {
        self.next()
            .ok_or_else(|| anyhow!("unexpected eol"))
            .and_then(|it| it.parse::<usize>().map_err(|e| anyhow!(e)))
            .context("expected usize")
    }

    fn take_tween(&mut self) -> Result<TweenId> {
        self.next()
            .ok_or_else(|| anyhow!("unexpected eol"))
            .and_then(|it| {
                let t = it.parse::<u8>().map_err(|e| anyhow!(e))?;
                Ok(RPE_TWEEN_MAP
                    .get(t as usize)
                    .copied()
                    .unwrap_or(RPE_TWEEN_MAP[0]))
            })
            .context("expected tween")
    }

    fn take_time(&mut self, b: &mut BpmList) -> Result<f32> {
        self.take_f32().map(|it| b.time_at_beats(it))
    }
}

struct PECEvent {
    pub start_time: f32,
    pub end_time: f32,
    pub end: f32,
    pub easing: TweenId,
}

impl PECEvent {
    pub fn new(start_time: f32, end_time: f32, end: f32, tween: TweenId) -> Self {
        Self {
            start_time,
            end_time,
            end,
            easing: tween,
        }
    }

    pub fn single(time: f32, value: f32) -> Self {
        Self::new(time, time, value, 0)
    }
}

#[derive(Default)]
struct PECJudgeLine {
    pub speed_events: Vec<(f32, f32)>,
    pub alpha_events: Vec<PECEvent>,
    pub move_events: (Vec<PECEvent>, Vec<PECEvent>),
    pub rotate_events: Vec<PECEvent>,
    pub notes: Vec<Note>,
}

fn sanitize_events(events: &mut [PECEvent], id: usize, desc: &str) {
    events.sort_by_key(|e| (e.end_time.not_nan(), e.start_time.not_nan()));
    let mut last_end = f32::NEG_INFINITY;
    for e in events.iter_mut() {
        if e.start_time < last_end {
            log::warn!(
                "Overlap detected in {} events for judge line {}: clipping from {} to {}",
                desc,
                id,
                e.start_time,
                last_end
            );
            e.start_time = last_end;
        }
        last_end = e.end_time;
    }
}

fn parse_events(mut events: Vec<PECEvent>, id: usize, desc: &str) -> Result<AnimFloat> {
    sanitize_events(&mut events, id, desc);
    let mut kfs = Vec::new();
    for e in events {
        if e.start_time == e.end_time {
            kfs.push(Keyframe::new(e.start_time, e.end, 0));
        } else {
            if kfs.is_empty() {
                bail!(
                    "failed to parse {} events: interpolating event found before a concrete value appears",
                    desc
                );
            }
            let last_val = kfs.last().unwrap().value;
            kfs.push(Keyframe::new(e.start_time, last_val, e.easing));
            kfs.push(Keyframe::new(e.end_time, e.end, 0));
        }
    }
    Ok(AnimFloat::new(kfs))
}

fn parse_speed_events(mut pec: Vec<(f32, f32)>, max_time: f32) -> AnimFloat {
    if pec.is_empty() {
        return AnimFloat::default();
    }
    if pec[0].0 >= EPS {
        pec.insert(0, (0., 0.));
    }
    let mut kfs = Vec::new();
    let mut height = 0.0;
    let mut last_time = 0.0;
    let mut last_speed = 0.0;
    for (time, speed) in pec {
        height += (time - last_time) * last_speed;
        kfs.push(Keyframe::new(time, height, 2));
        last_time = time;
        last_speed = speed;
    }
    kfs.push(Keyframe::new(
        max_time,
        height + (max_time - last_time) * last_speed,
        0,
    ));
    AnimFloat::new(kfs)
}

fn parse_judge_line(mut pec: PECJudgeLine, id: usize, max_time: f32) -> Result<JudgeLine> {
    let mut height = parse_speed_events(pec.speed_events, max_time);
    for note in &mut pec.notes {
        height.set_time(note.time);
        note.height = height.now();
        if let NoteKind::Hold {
            end_time,
            end_height,
        } = &mut note.kind
        {
            height.set_time(*end_time);
            *end_height = height.now();
        }
    }
    pec.move_events
        .0
        .iter_mut()
        .for_each(|it| it.end = it.end / 2048. * 2. - 1.);
    pec.move_events
        .1
        .iter_mut()
        .for_each(|it| it.end = it.end / 1400. * 2. - 1.);
    pec.alpha_events.iter_mut().for_each(|it| {
        if it.end >= 0.0 {
            it.end /= 255.;
        }
    });

    Ok(JudgeLine {
        object: Object {
            alpha: parse_events(pec.alpha_events, id, "alpha")?,
            translation: AnimVector {
                x: parse_events(pec.move_events.0, id, "move X")?,
                y: parse_events(pec.move_events.1, id, "move Y")?,
            },
            rotation: parse_events(pec.rotate_events, id, "rotate")?,
            scale: AnimVector {
                x: AnimFloat::fixed(3.91 / 6.0),
                y: AnimFloat::default(),
            },
        },
        ctrl_obj: monitor_common::core::CtrlObject::default(),
        kind: JudgeLineKind::Normal,
        height,
        incline: AnimFloat::default(),
        notes: pec.notes,
        color: Anim::default(),
        parent: None,
        z_index: 0,
        show_below: false,
        attach_ui: None,
    })
}

pub async fn parse_pec(source: &str) -> Result<Chart> {
    let mut offset = None;
    let mut b = None;
    let mut lines = Vec::new();
    let mut bpm_list = Vec::new();
    let mut last_line = None;

    fn get_line(lines: &mut Vec<PECJudgeLine>, id: usize) -> &mut PECJudgeLine {
        if lines.len() <= id {
            for _ in 0..=(id - lines.len()) {
                lines.push(PECJudgeLine::default());
            }
        }
        &mut lines[id]
    }

    fn ensure_bpm<'a>(
        b: &'a mut Option<BpmList>,
        bpm_list: &mut Vec<(f32, f32)>,
    ) -> &'a mut BpmList {
        if b.is_none() {
            *b = Some(BpmList::new(std::mem::take(bpm_list)));
        }
        b.as_mut().unwrap()
    }

    for (line_id, line_content) in source.lines().enumerate() {
        let mut it = line_content.split_whitespace();
        if offset.is_none() {
            offset = Some(it.take_f32()? / 1000. - 0.15);
        } else {
            let Some(cmd) = it.next() else {
                continue;
            };
            let cs: Vec<_> = cmd.chars().collect();
            match cs[0] {
                'b' if cmd == "bp" => {
                    if b.is_some() {
                        bail!("bp error at line {}", line_id + 1);
                    }
                    bpm_list.push((it.take_f32()?, it.take_f32()?));
                }
                'n' if cs.len() == 2 && ('1'..='4').contains(&cs[1]) => {
                    let b_ref = ensure_bpm(&mut b, &mut bpm_list);
                    let line_idx = it.take_usize()?;
                    last_line = Some(line_idx);
                    let p_line = get_line(&mut lines, line_idx);
                    let time = it.take_time(b_ref)?;
                    let kind = match cs[1] {
                        '1' => NoteKind::Click,
                        '2' => NoteKind::Hold {
                            end_time: it.take_time(b_ref)?,
                            end_height: 0.0,
                        },
                        '3' => NoteKind::Flick,
                        '4' => NoteKind::Drag,
                        _ => unreachable!(),
                    };
                    let position_x = it.take_f32()? / 1024.;
                    let above = it.take_usize()? == 1;
                    let fake = it.take_usize()? == 1;

                    p_line.notes.push(Note {
                        object: Object {
                            translation: AnimVector {
                                x: AnimFloat::fixed(position_x),
                                y: AnimFloat::default(),
                            },
                            ..Default::default()
                        },
                        kind,
                        hitsound: None,
                        time,
                        height: 0.0,
                        speed: 1.0,
                        above,
                        multiple_hint: false,
                        fake,
                    });

                    let mut it_clone = it.clone();
                    if it_clone.next() == Some("#") {
                        it.next();
                        lines[line_idx].notes.last_mut().unwrap().speed = it.take_f32()?;
                    }
                    it_clone = it.clone();
                    if it_clone.next() == Some("&") {
                        it.next();
                        let size = it.take_f32()?;
                        if (size - 1.0).abs() >= EPS {
                            lines[line_idx].notes.last_mut().unwrap().object.scale.x =
                                AnimFloat::fixed(size);
                        }
                    }
                }
                '#' if cs.len() == 1 => {
                    if let Some(ll) = last_line {
                        lines[ll].notes.last_mut().unwrap().speed = it.take_f32()?;
                    }
                }
                '&' if cs.len() == 1 => {
                    if let Some(ll) = last_line {
                        let size = it.take_f32()?;
                        if (size - 1.0).abs() >= EPS {
                            lines[ll].notes.last_mut().unwrap().object.scale.x =
                                AnimFloat::fixed(size);
                        }
                    }
                }
                'c' if cs.len() == 2 => {
                    let b_ref = ensure_bpm(&mut b, &mut bpm_list);
                    let line_idx = it.take_usize()?;
                    let p_line = get_line(&mut lines, line_idx);
                    let time = it.take_time(b_ref)?;
                    match cs[1] {
                        'v' => {
                            p_line.speed_events.push((time, it.take_f32()? / 5.85));
                        }
                        'p' => {
                            let x = it.take_f32()?;
                            let y = it.take_f32()?;
                            p_line.move_events.0.push(PECEvent::single(time, x));
                            p_line.move_events.1.push(PECEvent::single(time, y));
                        }
                        'd' => {
                            p_line
                                .rotate_events
                                .push(PECEvent::single(time, -it.take_f32()?));
                        }
                        'a' => {
                            p_line
                                .alpha_events
                                .push(PECEvent::single(time, it.take_f32()?));
                        }
                        'm' => {
                            let end_time = it.take_time(b_ref)?;
                            let x = it.take_f32()?;
                            let y = it.take_f32()?;
                            let t = it.take_tween()?;
                            p_line
                                .move_events
                                .0
                                .push(PECEvent::new(time, end_time, x, t));
                            p_line
                                .move_events
                                .1
                                .push(PECEvent::new(time, end_time, y, t));
                        }
                        'r' => {
                            p_line.rotate_events.push(PECEvent::new(
                                time,
                                it.take_time(b_ref)?,
                                -it.take_f32()?,
                                it.take_tween()?,
                            ));
                        }
                        'f' => {
                            p_line.alpha_events.push(PECEvent::new(
                                time,
                                it.take_time(b_ref)?,
                                it.take_f32()?,
                                2,
                            ));
                        }
                        _ => bail!("unknown command {} at line {}", cmd, line_id + 1),
                    }
                }
                _ => bail!("unknown command {} at line {}", cmd, line_id + 1),
            }
        }
    }

    let max_time = *lines
        .iter()
        .map(|it| {
            it.alpha_events
                .iter()
                .chain(it.rotate_events.iter())
                .chain(it.move_events.0.iter())
                .chain(it.move_events.1.iter())
                .map(|it| it.end_time.not_nan())
                .chain(it.speed_events.iter().map(|it| it.0.not_nan()))
                .chain(it.notes.iter().map(|it| it.time.not_nan()))
                .max()
                .unwrap_or_default()
        })
        .max()
        .unwrap_or_default()
        + 1.;

    let mut final_lines = lines
        .into_iter()
        .enumerate()
        .map(|(id, line)| parse_judge_line(line, id, max_time))
        .collect::<Result<Vec<_>>>()?;

    process_lines(&mut final_lines);
    ensure_bpm(&mut b, &mut bpm_list);
    Ok(Chart::new(
        offset.unwrap_or(0.),
        final_lines,
        b.unwrap_or_default(),
    ))
}
