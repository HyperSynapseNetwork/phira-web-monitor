use super::process_lines;
use anyhow::{bail, Result};
use byteorder::{LittleEndian as LE, ReadBytesExt};
use monitor_common::core::{
    Anim, AnimVector, BezierTween, BpmList, Chart, ChartSettings, ClampedTween, CtrlObject,
    JudgeLine, JudgeLineKind, Keyframe, Note, NoteKind, Object, Texture, TweenFn, Tweenable,
    UIElement,
};
use std::io::Read;

pub struct BinaryReader<R: Read> {
    reader: R,
    time_cursor: u32,
}

impl<R: Read> BinaryReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            time_cursor: 0,
        }
    }

    pub fn reset_time(&mut self) {
        self.time_cursor = 0;
    }

    pub fn time(&mut self) -> Result<f32> {
        self.time_cursor += self.uleb()? as u32;
        Ok(self.time_cursor as f32 / 1000.0)
    }

    pub fn uleb(&mut self) -> Result<u64> {
        let mut result = 0;
        let mut shift = 0;
        loop {
            let byte = self.reader.read_u8()?;
            result |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                break Ok(result);
            }
            shift += 7;
        }
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        Ok(self.reader.read_u8()?)
    }

    pub fn read_i32(&mut self) -> Result<i32> {
        Ok(self.reader.read_i32::<LE>()?)
    }

    pub fn read_f32(&mut self) -> Result<f32> {
        Ok(self.reader.read_f32::<LE>()?)
    }

    pub fn read_bool(&mut self) -> Result<bool> {
        Ok(self.reader.read_u8()? == 1)
    }

    pub fn read_string(&mut self) -> Result<String> {
        let len = self.uleb()? as usize;
        let mut buf = vec![0u8; len];
        self.reader.read_exact(&mut buf)?;
        Ok(String::from_utf8(buf)?)
    }

    pub fn read_array<T, F>(&mut self, mut f: F) -> Result<Vec<T>>
    where
        F: FnMut(&mut Self) -> Result<T>,
    {
        let len = self.uleb()? as usize;
        let mut vec = Vec::with_capacity(len);
        for _ in 0..len {
            vec.push(f(self)?);
        }
        Ok(vec)
    }
}

fn read_keyframe<T: BinaryRead>(r: &mut BinaryReader<impl Read>) -> Result<Keyframe<T>> {
    let time = r.time()?;
    let value = T::read_binary(r)?;
    let b = r.read_u8()?;
    let tween = match b & 0xC0 {
        0 => TweenFn::TweenId(b),
        0x80 => {
            let start = r.read_f32()?;
            let end = r.read_f32()?;
            TweenFn::Clamped(ClampedTween::new(b & 0x7f, start..end))
        }
        0xC0 => {
            let p1x = r.read_f32()?;
            let p1y = r.read_f32()?;
            let p2x = r.read_f32()?;
            let p2y = r.read_f32()?;
            TweenFn::Bezier(BezierTween::new((p1x, p1y), (p2x, p2y)))
        }
        _ => bail!("invalid tween tag"),
    };
    Ok(Keyframe { time, value, tween })
}

fn read_anim<T: BinaryRead + Tweenable>(
    r: &mut BinaryReader<impl Read>,
) -> Result<Option<Box<Anim<T>>>> {
    Ok(match r.read_u8()? {
        0 => None,
        x => {
            let mut res = if x == 1 {
                Anim::default()
            } else {
                r.reset_time();
                let kfs = r.read_array(|r| read_keyframe(r))?;
                Anim::new(kfs)
            };
            res.next = read_anim(r)?;
            Some(Box::new(res))
        }
    })
}

trait BinaryRead: Sized {
    fn read_binary(r: &mut BinaryReader<impl Read>) -> Result<Self>;
}

impl BinaryRead for f32 {
    fn read_binary(r: &mut BinaryReader<impl Read>) -> Result<Self> {
        r.read_f32()
    }
}

impl BinaryRead for String {
    fn read_binary(r: &mut BinaryReader<impl Read>) -> Result<Self> {
        r.read_string()
    }
}

impl BinaryRead for Object {
    fn read_binary(r: &mut BinaryReader<impl Read>) -> Result<Self> {
        Ok(Self {
            alpha: *read_anim(r)?.unwrap_or_default(),
            scale: AnimVector {
                x: *read_anim(r)?.unwrap_or_default(),
                y: *read_anim(r)?.unwrap_or_default(),
            },
            rotation: *read_anim(r)?.unwrap_or_default(),
            translation: AnimVector {
                x: *read_anim(r)?.unwrap_or_default(),
                y: *read_anim(r)?.unwrap_or_default(),
            },
        })
    }
}

fn read_note(r: &mut BinaryReader<impl Read>) -> Result<Note> {
    let object = Object::read_binary(r)?;
    let kind = match r.read_u8()? {
        0 => NoteKind::Click,
        1 => NoteKind::Hold {
            end_time: r.read_f32()?,
            end_height: r.read_f32()?,
        },
        2 => NoteKind::Flick,
        3 => NoteKind::Drag,
        _ => bail!("invalid note kind"),
    };
    Ok(Note {
        object,
        kind,
        hitsound: None,
        time: r.time()?,
        height: r.read_f32()?,
        speed: if r.read_bool()? { r.read_f32()? } else { 1.0 },
        above: r.read_bool()?,
        multiple_hint: false,
        fake: r.read_bool()?,
    })
}

fn read_judge_line(r: &mut BinaryReader<impl Read>) -> Result<JudgeLine> {
    r.reset_time();
    let object = Object::read_binary(r)?;
    let kind = match r.read_u8()? {
        0 => JudgeLineKind::Normal,
        1 => JudgeLineKind::Texture(Texture::empty().into(), r.read_string()?),
        2 => JudgeLineKind::Text(*read_anim::<String>(r)?.unwrap_or_default()),
        3 => JudgeLineKind::Paint(*read_anim::<f32>(r)?.unwrap_or_default()),
        _ => bail!("invalid judge line kind"),
    };
    let height = *read_anim::<f32>(r)?.unwrap_or_default();
    let notes = r.read_array(|r| read_note(r))?;

    // Skip color (4 u8s)
    let _ = r.read_u8()?;
    let _ = r.read_u8()?;
    let _ = r.read_u8()?;
    let _ = r.read_u8()?;

    let parent = match r.uleb()? {
        0 => None,
        x => Some(x as usize - 1),
    };
    let show_below = r.read_bool()?;
    let u = r.read_u8()?;
    let attach_ui = if u == 0 {
        None
    } else {
        match u {
            1 => Some(UIElement::Pause),
            2 => Some(UIElement::ComboNumber),
            3 => Some(UIElement::Combo),
            4 => Some(UIElement::Score),
            5 => Some(UIElement::Bar),
            6 => Some(UIElement::Name),
            7 => Some(UIElement::Level),
            _ => None,
        }
    };

    assert_eq!(r.read_u8()?, 8);
    let ctrl_obj = CtrlObject {
        alpha: *read_anim::<f32>(r)?.unwrap_or_default(),
        size: *read_anim::<f32>(r)?.unwrap_or_default(),
        pos: *read_anim::<f32>(r)?.unwrap_or_default(),
        y: *read_anim::<f32>(r)?.unwrap_or_default(),
    };

    let incline = *read_anim::<f32>(r)?.unwrap_or_default();
    let z_index = r.read_i32()?;

    Ok(JudgeLine {
        object,
        kind,
        height,
        notes,
        color: Anim::default(),
        parent,
        show_below,
        attach_ui,
        ctrl_obj,
        incline,
        z_index,
    })
}

pub async fn parse_pbc(source: &[u8]) -> Result<Chart> {
    let mut r = BinaryReader::new(source);
    let offset = r.read_f32()?;
    let mut lines = r.read_array(|r| read_judge_line(r))?;
    process_lines(&mut lines);
    let mut chart = Chart::new(offset, lines, BpmList::default());
    chart.settings = ChartSettings {
        pe_alpha_extension: r.read_bool()?,
        hold_partial_cover: r.read_bool()?,
    };
    Ok(chart)
}
