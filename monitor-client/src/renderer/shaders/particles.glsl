#ifdef VERTEX
layout(location = 0) in vec3 a_pos;          // Quad vertex position
layout(location = 1) in vec2 a_uv;           // Quad UV
layout(location = 2) in vec4 a_color;        // Color (unused?)
layout(location = 3) in vec4 a_inst_pos;     // Instance: x, y, rotation, size
layout(location = 4) in vec4 a_inst_uv;      // Instance: u, v, w, h
layout(location = 5) in vec4 a_inst_data;    // Instance: index, lifetime, 0, 0
layout(location = 6) in vec4 a_inst_color;   // Instance: r, g, b, a

uniform mat4 u_mvp;
uniform float u_local_coords;       // 0.0 = global, 1.0 = local
uniform vec3 u_emitter_position;    // Position of emitter

out vec2 v_uv;
out vec4 v_color;

mat2 rotate2d(float angle) {
    return mat2(cos(angle), -sin(angle),
                sin(angle), cos(angle));
}

void main() {
    float rotation = a_inst_pos.z;
    float size = a_inst_pos.w;
    vec2 pos = a_inst_pos.xy;

    mat2 rot = rotate2d(rotation);
    
    // Rotate and scale the quad vertex
    vec2 transformed_pos = rot * a_pos.xy * size;
    
    // Apply instance position
    vec3 final_pos;
    if (u_local_coords == 0.0) {
        // Global coordinates
        final_pos = vec3(transformed_pos + pos, a_pos.z);
    } else {
        // Local coordinates (relative to emitter)
        final_pos = vec3(transformed_pos + pos + u_emitter_position.xy, a_pos.z);
    }

    gl_Position = u_mvp * vec4(final_pos, 1.0);
    
    // Transform UVs based on atlas
    v_uv = a_uv * a_inst_uv.zw + a_inst_uv.xy;
    
    // Pass color
    v_color = a_inst_color;
}
#endif

#ifdef FRAGMENT
precision mediump float;

in vec2 v_uv;
in vec4 v_color;

uniform sampler2D u_texture;

out vec4 out_color;

void main() {
    out_color = texture(u_texture, v_uv) * v_color;
}
#endif
