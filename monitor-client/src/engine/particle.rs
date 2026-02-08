use crate::renderer::{Renderer, Texture};
use nalgebra::Vector2;

pub struct Particle {
    pub pos: Vector2<f32>,
    pub vel: Vector2<f32>,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub color: [f32; 4],
}

pub struct ParticleSystem {
    pub particles: Vec<Particle>,
    pub texture: Option<Texture>,
}

impl ParticleSystem {
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            texture: None,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.particles.retain_mut(|p| {
            p.life -= dt;
            p.pos += p.vel * dt;
            p.life > 0.0
        });
    }

    pub fn render(&self, renderer: &mut Renderer) {
        if let Some(tex) = &self.texture {
            renderer.set_texture(tex);
        } else {
            renderer.set_texture(&renderer.white_texture.clone());
        }

        for p in &self.particles {
            let alpha = p.color[3] * (p.life / p.max_life);
            renderer.draw_texture_rect(
                p.pos.x - p.size / 2.0,
                p.pos.y - p.size / 2.0,
                p.size,
                p.size,
                0.0,
                0.0,
                1.0,
                1.0,
                p.color[0],
                p.color[1],
                p.color[2],
                alpha,
                &[
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                ],
            );
        }
    }

    pub fn emit(
        &mut self,
        pos: Vector2<f32>,
        vel: Vector2<f32>,
        life: f32,
        size: f32,
        color: [f32; 4],
    ) {
        self.particles.push(Particle {
            pos,
            vel,
            life,
            max_life: life,
            size,
            color,
        });
    }
}
