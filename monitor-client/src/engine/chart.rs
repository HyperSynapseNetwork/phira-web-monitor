use crate::engine::{line::draw_line, particle::ParticleSystem, resource::Resource};
use crate::renderer::Renderer;
use monitor_common::core::Chart;

pub struct ChartRenderer {
    pub chart: Chart,
    pub resource: Resource,
    pub particle_system: ParticleSystem,
    pub time: f32, // Seconds
}

impl ChartRenderer {
    pub fn new(chart: Chart, resource: Resource) -> Self {
        Self {
            chart,
            resource,
            particle_system: ParticleSystem::new(),
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
        self.chart.set_time(time);

        if dt > 0.0 {
            self.particle_system.update(dt);
        }
    }

    pub fn render(&mut self, renderer: &mut Renderer) {
        for line in &self.chart.lines {
            draw_line(&mut self.resource, line, renderer);
        }

        self.particle_system.render(renderer);
    }
}
