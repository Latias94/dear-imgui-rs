#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct CompositionSettings {
    intensity: f32,
    _padding: vec3<f32>,
}

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: CompositionSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let source = textureSample(source_texture, source_sampler, in.uv);
    let luminance = dot(source.rgb, vec3(0.2126, 0.7152, 0.0722));
    let cool_grade = vec3(
        luminance * 0.72 + source.r * 0.28,
        source.g * 0.86 + luminance * 0.14,
        source.b * 1.12 + 0.025,
    );
    return vec4(mix(source.rgb, cool_grade, settings.intensity), source.a);
}
