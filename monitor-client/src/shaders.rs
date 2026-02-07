//! GLSL Shaders
//!
//! Basic shaders for 2D rendering.

pub const VERTEX_SHADER: &str = r#"#version 300 es
in vec2 a_position;
in vec2 a_uv;
in vec4 a_color;

uniform vec2 u_resolution;

out vec4 v_color;
out vec2 v_uv;

void main() {
    // Convert position to clip space
    vec2 zero_to_one = a_position / u_resolution;
    vec2 zero_to_two = zero_to_one * 2.0;
    vec2 clip_space = zero_to_two - 1.0;
    
    gl_Position = vec4(clip_space * vec2(1, -1), 0, 1);
    v_color = a_color;
    v_uv = a_uv;
}
"#;

pub const FRAGMENT_SHADER: &str = r#"#version 300 es
precision mediump float;

in vec4 v_color;
in vec2 v_uv;

uniform sampler2D u_texture;

out vec4 out_color;

void main() {
    out_color = v_color * texture(u_texture, v_uv);
}
"#;
