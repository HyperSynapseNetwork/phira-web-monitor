use crate::engine::{line::draw_line, resource::Resource};
use crate::renderer::Renderer;
use monitor_common::core::{Chart, ChartInfo, Matrix, Vector};
use nalgebra::{Matrix3, Rotation2};

pub struct ChartRenderer {
    pub info: ChartInfo,
    pub chart: Chart,
    pub resource: Resource,
    pub time: f32, // Seconds
    pub world_matrices: Vec<Option<Matrix>>,
}

impl ChartRenderer {
    pub fn new(info: ChartInfo, chart: Chart, resource: Resource) -> Self {
        let n = chart.lines.len();
        Self {
            info,
            chart,
            resource,
            time: 0.0,
            world_matrices: vec![None; n],
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.resource.width = width;
        self.resource.height = height;
        self.resource.aspect_ratio = width as f32 / height as f32;
    }

    fn fetch_pos(&self, line_index: usize) -> Vector {
        let line = &self.chart.lines[line_index];
        if let Some(parent) = line.parent {
            let parent_translation = self.fetch_pos(parent);
            let parent_line = &self.chart.lines[parent];
            let parent_rotation = parent_line.object.rotation.now_opt().unwrap_or(0.0);
            return parent_translation
                + Rotation2::new(parent_rotation.to_radians())
                    * line.object.now_translation(self.resource.aspect_ratio);
        }
        line.object.now_translation(self.resource.aspect_ratio)
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

    pub fn update(&mut self, time: f32) {
        let dt = time - self.time;
        self.time = time;
        self.resource.time = time;
        self.resource.dt = dt;
        self.chart.set_time(time);

        // Calculate world matrices
        self.world_matrices.fill(None);
        for i in 0..self.chart.lines.len() {
            self.world_matrices[i] = Some(self.fetch_transform(i));
        }
    }

    pub fn render(&mut self, renderer: &mut Renderer) {
        for &i in &self.chart.order {
            let line = &self.chart.lines[i];
            let world_matrix = self.world_matrices[i].unwrap_or(Matrix::identity());
            draw_line(
                &mut self.resource,
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
        if let Some(emitter) = &mut self.resource.emitter {
            emitter.draw(renderer, self.resource.dt);
        }
    }
}
