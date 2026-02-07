use crate::renderer::Renderer;

pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub color: (f32, f32, f32), // RGB
}

pub struct ParticleSystem {
    particles: Vec<Particle>,
}

impl ParticleSystem {
    pub fn new() -> Self {
        Self {
            particles: Vec::with_capacity(100),
        }
    }

    pub fn spawn(&mut self, x: f32, y: f32, color: (f32, f32, f32)) {
        // Spawn a burst of particles
        let count = 10;
        for _ in 0..count {
            let angle = js_sys::Math::random() * std::f64::consts::TAU;
            let speed = 50.0 + js_sys::Math::random() * 150.0;
            let vx = (angle.cos() * speed) as f32;
            let vy = (angle.sin() * speed) as f32;

            self.particles.push(Particle {
                x,
                y,
                vx,
                vy,
                life: 0.5 + (js_sys::Math::random() as f32) * 0.3,
                max_life: 0.8,
                size: 5.0 + (js_sys::Math::random() as f32) * 10.0,
                color,
            });
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.particles.retain_mut(|p| {
            p.life -= dt;
            if p.life <= 0.0 {
                return false;
            }

            p.x += p.vx * dt;
            p.y += p.vy * dt;
            // Simple gravity/drag could go here

            true
        });
    }

    pub fn render(&self, renderer: &mut Renderer) {
        for p in &self.particles {
            let alpha = p.life / p.max_life;
            let angle = 0.0; // Particles don't have an angle property yet, default to 0.0
            renderer.draw_rotated_rect(
                p.x, p.y, p.size, p.size, angle, 0.0, 0.0, 1.0, 1.0, // UVs
                p.color.0, p.color.1, p.color.2, alpha,
            );
        }
    }
}
