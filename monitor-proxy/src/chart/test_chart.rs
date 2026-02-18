use anyhow::Result;
use monitor_common::core::{AnimFloat, Chart, ChartInfo, JudgeLine, Keyframe, Note, NoteKind};

pub fn generate_test_chart() -> Result<(ChartInfo, Chart)> {
    let mut line = JudgeLine::default();
    const HEIGHT_PER_SEC: f32 = 1.0;

    line.height = AnimFloat::new(vec![
        Keyframe::new(0.0, 0.0, 2),
        Keyframe::new(100.0, 100.0 * HEIGHT_PER_SEC, 0),
    ]);

    let mut add_note = |kind: NoteKind, time: f32| {
        let h = time * HEIGHT_PER_SEC;
        line.notes.push(Note {
            kind,
            time,
            height: h,
            speed: 1.0,
            ..Default::default()
        });
    };

    add_note(NoteKind::Click, 2.0);
    add_note(NoteKind::Click, 3.0);
    add_note(NoteKind::Drag, 3.5);
    add_note(NoteKind::Flick, 4.0);

    let start_t = 5.0;
    let end_t = 7.0;
    line.notes.push(Note {
        kind: NoteKind::Hold {
            end_time: end_t,
            end_height: end_t * HEIGHT_PER_SEC,
        },
        time: start_t,
        height: start_t * HEIGHT_PER_SEC,
        speed: 1.0,
        ..Default::default()
    });

    let info = ChartInfo::default();
    let chart = Chart {
        offset: 0.0,
        lines: vec![line],
        ..Default::default()
    };

    Ok((info, chart))
}
