// Dear ImGui WGSL shader for Bevy
// Based on the original Dear ImGui GLSL shaders and bevy_egui implementation

struct Transform {
    scale: vec2<f32>,
    translation: vec2<f32>,
}

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: u32,  // Dear ImGui uses packed RGBA u32 color
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

    // Unpack u32 color to vec4<f32> (RGBA)
    let color = vec4<f32>(
        f32((in.color >> 0u) & 0xFFu) / 255.0,   // Red
        f32((in.color >> 8u) & 0xFFu) / 255.0,   // Green
        f32((in.color >> 16u) & 0xFFu) / 255.0,  // Blue
        f32((in.color >> 24u) & 0xFFu) / 255.0,  // Alpha
    );

    return VertexOutput(vec4<f32>(position, 0.0, 1.0), color, in.uv);
}

// sRGB to linear conversion function
fn srgb_to_linear(srgb: vec4<f32>) -> vec4<f32> {
    let color_srgb = srgb.rgb;
    let selector = ceil(color_srgb - 0.04045); // 0 if under value, 1 if over
    let under = color_srgb / 12.92;
    let over = pow((color_srgb + 0.055) / 1.055, vec3<f32>(2.4));
    let result = mix(under, over, selector);
    return vec4<f32>(result, srgb.a);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let texture_color = textureSample(image_texture, image_sampler, in.uv);
    // Convert vertex color from sRGB to linear space for proper blending
    let linear_color = srgb_to_linear(in.color);
    return texture_color * linear_color;
}
