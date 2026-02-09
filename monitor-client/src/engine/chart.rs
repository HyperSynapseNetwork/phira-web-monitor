use crate::engine::{line::draw_line, resource::Resource};
use crate::renderer::Renderer;
use monitor_common::core::Chart;

pub struct ChartRenderer {
    pub chart: Chart,
    pub resource: Resource,
    pub time: f32, // Seconds
}

impl ChartRenderer {
    pub fn new(chart: Chart, resource: Resource) -> Self {
        Self {
            chart,
            resource,
            time: 0.0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.resource.width = width;
        self.resource.height = height;
        self.resource.aspect_ratio = width as f32 / height as f32;
    }

    pub fn update(&mut self, time: f32) {
        let dt = time - self.time;
        self.time = time;
        self.resource.time = time;
        self.resource.dt = dt;
        self.chart.set_time(time);
    }

    pub fn render(&mut self, renderer: &mut Renderer) {
        for (i, line) in self.chart.lines.iter().enumerate() {
            draw_line(&mut self.resource, line, renderer, i, &self.chart.settings);
        }

        // Flush lines before drawing particles to avoid state leaks
        renderer.flush();

        if let Some(emitter) = &mut self.resource.emitter {
            emitter.draw(renderer, self.resource.dt);
        }
    }
}
