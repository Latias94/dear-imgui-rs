// Dear ImGui WGSL shader for Bevy
// Based on the original Dear ImGui GLSL shaders and bevy_egui implementation

struct Transform {
    scale: vec2<f32>,
    translation: vec2<f32>,
}

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

@group(0) @binding(0) var<uniform> transform: Transform;
@group(1) @binding(0) var image_texture: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let position = in.position * transform.scale + transform.translation;
    return VertexOutput(vec4<f32>(position, 0.0, 1.0), in.color, in.uv);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let texture_color = textureSample(image_texture, image_sampler, in.uv);
    return texture_color * in.color;
}
