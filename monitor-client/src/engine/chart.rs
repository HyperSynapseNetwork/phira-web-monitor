use crate::engine::judge::{JudgeEvent, JudgeEventKind};
use crate::engine::{Resource, draw_line};
use crate::renderer::Renderer;
use monitor_common::core::{Chart, ChartInfo, JudgeStatus, Judgement, Matrix, NoteKind, Vector};
use nalgebra::{Matrix3, Rotation2};
use std::f32::consts::PI;

const HOLD_PARTICLE_INTERVAL: f32 = 0.15;

pub struct ChartRenderer {
    pub info: ChartInfo,
    pub chart: Chart,
    pub time: f32, // Seconds
    pub world_matrices: Vec<Option<Matrix>>,
    pub autoplay: bool,
}

impl ChartRenderer {
    pub fn new(info: ChartInfo, chart: Chart) -> Self {
        let n = chart.lines.len();
        Self {
            info,
            chart,
            time: 0.0,
            world_matrices: vec![None; n],
            autoplay: true,
        }
    }

    fn fetch_pos(&self, line_index: usize) -> Vector {
        let line = &self.chart.lines[line_index];
        if let Some(parent) = line.parent {
            let parent_translation = self.fetch_pos(parent);
            let parent_line = &self.chart.lines[parent];
            let parent_rotation = parent_line.object.rotation.now_opt().unwrap_or(0.0);
            return parent_translation
                + Rotation2::new(parent_rotation.to_radians())
                    * line.object.now_translation(self.info.aspect_ratio);
        }
        line.object.now_translation(self.info.aspect_ratio)
    }

    fn fetch_transform(&self, line_index: usize) -> Matrix {
        if let Some(matrix) = self.world_matrices[line_index] {
            return matrix;
        }
        let line = &self.chart.lines[line_index];
        let translation = self.fetch_pos(line_index);
        let rot = line.object.rotation.now_opt().unwrap_or(0.0);
        let rotation = Rotation2::new(rot.to_radians());

        let mut transform = Matrix3::identity();
        transform
            .fixed_view_mut::<2, 2>(0, 0)
            .copy_from(rotation.matrix());
        transform[(0, 2)] = translation.x;
        transform[(1, 2)] = translation.y;
        transform
    }

    pub fn update(&mut self, res: &mut Resource, time: f32) {
        let dt = time - self.time;
        self.time = time;
        res.time = time;
        res.dt = dt;
        self.chart.set_time(time);

        // Calculate world matrices
        self.world_matrices.fill(None);
        for i in 0..self.chart.lines.len() {
            self.world_matrices[i] = Some(self.fetch_transform(i));
        }
    }

    /// Judge update pass. Returns events for hitsound/particle consumption.
    /// Must be called after `update()` and before `render()`.
    pub fn update_judges(&mut self, res: &Resource) -> Vec<JudgeEvent> {
        let t = res.time;
        let dt = res.dt;
        let mut events = Vec::new();

        // No judge updates when paused or seeking backward
        if dt <= 0.0 {
            return events;
        }

        for (line_idx, line) in self.chart.lines.iter_mut().enumerate() {
            for (note_idx, note) in line.notes.iter_mut().enumerate() {
                if note.fake {
                    continue;
                }

                match &note.judge {
                    JudgeStatus::NotJudged => {
                        if self.autoplay && note.time <= t {
                            match &note.kind {
                                NoteKind::Hold { .. } => {
                                    note.judge =
                                        JudgeStatus::Hold(true, t, 0.0, false, f32::INFINITY);
                                    events.push(JudgeEvent {
                                        kind: JudgeEventKind::HoldStart,
                                        line_idx,
                                        note_idx,
                                    });
                                }
                                _ => {
                                    note.judge = JudgeStatus::Judged;
                                    events.push(JudgeEvent {
                                        kind: JudgeEventKind::Judged(Judgement::Perfect),
                                        line_idx,
                                        note_idx,
                                    });
                                }
                            }
                        } else if !self.autoplay && t - note.time > 0.22 {
                            // Miss (LIMIT_BAD)
                            note.judge = JudgeStatus::Judged;
                        }
                    }
                    JudgeStatus::Hold(perfect, at, diff, pre_judge, up_time) => {
                        if let NoteKind::Hold { end_time, .. } = &note.kind {
                            if t >= *end_time {
                                let j = if *perfect {
                                    Judgement::Perfect
                                } else {
                                    Judgement::Good
                                };
                                events.push(JudgeEvent {
                                    kind: JudgeEventKind::HoldComplete(j),
                                    line_idx,
                                    note_idx,
                                });
                                note.judge = JudgeStatus::Judged;
                            } else if t > *at {
                                // Advance particle tick timer
                                let j = if *perfect {
                                    Judgement::Perfect
                                } else {
                                    Judgement::Good
                                };
                                // Reconstruct to update `at`
                                note.judge = JudgeStatus::Hold(
                                    *perfect,
                                    *at + HOLD_PARTICLE_INTERVAL,
                                    *diff,
                                    *pre_judge,
                                    *up_time,
                                );
                                events.push(JudgeEvent {
                                    kind: JudgeEventKind::HoldTick(j),
                                    line_idx,
                                    note_idx,
                                });
                            }
                        }
                    }
                    _ => {} // Judged, PreJudge — no action
                }
            }
        }

        events
    }

    pub fn render(&mut self, res: &mut Resource, renderer: &mut Renderer) {
        for &i in &self.chart.order {
            let line = &self.chart.lines[i];
            let world_matrix = self.world_matrices[i].unwrap_or(Matrix::identity());
            draw_line(
                res,
                line,
                self.info.line_length,
                renderer,
                i,
                &self.chart.settings,
                world_matrix,
            );
        }

        // Flush lines before drawing particles to avoid state leaks
        renderer.flush();
        if let Some(emitter) = &mut res.emitter {
            emitter.draw(renderer, res.dt);
        }
    }

    /// Emit particles for judge events. Must be called after `update_judges()`
    /// and before `render()` so particles appear on the correct frame.
    pub fn emit_particles(&self, res: &mut Resource, events: &[JudgeEvent]) {
        for event in events {
            let color = match &event.kind {
                JudgeEventKind::Judged(j)
                | JudgeEventKind::HoldTick(j)
                | JudgeEventKind::HoldComplete(j) => {
                    if let Some(info) = res.res_pack.as_ref().map(|p| &p.info) {
                        match j {
                            Judgement::Perfect => info.fx_perfect(),
                            Judgement::Good => info.fx_good(),
                            _ => continue, // Bad/Miss — no particle
                        }
                    } else {
                        continue;
                    }
                }
                JudgeEventKind::HoldStart => continue, // No particle on hold start
            };

            let note = &self.chart.lines[event.line_idx].notes[event.note_idx];
            let line_matrix = self.world_matrices[event.line_idx].unwrap_or(Matrix::identity());

            // Note x position relative to line
            let note_x = note.object.translation.x.now_opt().unwrap_or(0.0);
            let note_offset = Matrix3::new_translation(&Vector::new(note_x, 0.0));

            let rotation = if note.above { 0.0 } else { PI };

            res.with_model(line_matrix * note_offset, |res| {
                res.emit_at_origin(rotation, color);
            });
        }
    }
}
