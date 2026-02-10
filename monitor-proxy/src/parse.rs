pub mod extra;
pub mod pbc;
pub mod pec;
pub mod pgr;
pub mod rpe;

use anyhow::Result;
use std::future::Future;
use std::pin::Pin;

/// Resource loader trait to abstract file system
pub trait ResourceLoader: Send + Sync {
    fn load_file<'a>(
        &'a mut self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;
}

use monitor_common::core::{easing_from, JudgeLine, TweenId, TweenMajor, TweenMinor};
use std::cmp::Ordering;

pub(crate) fn process_lines(v: &mut [JudgeLine]) {
    let mut times = Vec::new();
    // TODO optimize using k-merge sort
    let sorts = v
        .iter()
        .map(|line| {
            let mut idx: Vec<usize> = (0..line.notes.len()).collect();
            idx.sort_by(|&a, &b| {
                line.notes[a]
                    .time
                    .partial_cmp(&line.notes[b].time)
                    .unwrap_or(Ordering::Equal)
            });
            idx
        })
        .collect::<Vec<_>>();
    for (line, idx) in v.iter_mut().zip(sorts.iter()) {
        let v = &mut line.notes;
        let mut i = 0;
        while i < v.len() {
            times.push(v[idx[i]].time);
            let mut j = i + 1;
            while j < v.len() && v[idx[j]].time == v[idx[i]].time {
                j += 1;
            }
            if j != i + 1 {
                times.push(v[idx[i]].time);
            }
            i = j;
        }
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let mut mt = Vec::new();
    if !times.is_empty() {
        for i in 0..(times.len() - 1) {
            // since times are generated in the same way, theoretically we can compare them directly
            if times[i] == times[i + 1] && (i == 0 || times[i - 1] != times[i]) {
                mt.push(times[i]);
            }
        }
    }
    for (line, idx) in v.iter_mut().zip(sorts.iter()) {
        let mut i = 0;
        for id in idx {
            let note = &mut line.notes[*id];
            let time = note.time;
            while i < mt.len() && mt[i] < time {
                i += 1;
            }
            if i < mt.len() && mt[i] == time {
                note.multiple_hint = true;
            }
        }
    }
}

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
