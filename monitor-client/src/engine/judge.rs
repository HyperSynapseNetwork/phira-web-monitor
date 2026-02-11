use monitor_common::core::Judgement;

/// An event produced by the judge update pass, to be consumed
/// by the caller for hitsound playback and particle emission.
pub struct JudgeEvent {
    pub kind: JudgeEventKind,
    pub line_idx: usize,
    pub note_idx: usize,
}

pub enum JudgeEventKind {
    /// Click/Drag/Flick hit — emit particle + play hitsound
    Judged(Judgement),
    /// Hold started — play hitsound only (particles come from HoldTick)
    HoldStart,
    /// Hold tick — emit hold particle (no hitsound)
    HoldTick(Judgement),
    /// Hold completed — final judge committed
    HoldComplete(Judgement),
}
