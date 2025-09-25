// Dear ImGui WGSL Shader
// Vertex and fragment shaders for rendering Dear ImGui draw data

struct VertexInput {
    @location(0) a_Pos: vec2<f32>,
    @location(1) a_UV: vec2<f32>,
    @location(2) a_Color: vec4<f32>,
}

struct VertexOutput {
    @location(0) v_UV: vec2<f32>,
    @location(1) v_Color: vec4<f32>,
    @builtin(position) v_Position: vec4<f32>,
}

struct Uniforms {
    u_Matrix: mat4x4<f32>,
    u_Gamma: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var u_Sampler: sampler;

@group(1) @binding(0)
var u_Texture: texture_2d<f32>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.v_UV = in.a_UV;
    out.v_Color = in.a_Color;
    out.v_Position = uniforms.u_Matrix * vec4<f32>(in.a_Pos.xy, 0.0, 1.0);
    return out;
}

struct FragmentOutput {
    @location(0) o_Target: vec4<f32>,
}

fn srgb_to_linear(srgb: vec4<f32>) -> vec4<f32> {
    let color_srgb = srgb.rgb;
    let selector = ceil(color_srgb - 0.04045); // 0 if under value, 1 if over
    let under = color_srgb / 12.92;
    let over = pow((color_srgb + 0.055) / 1.055, vec3<f32>(2.4));
    let result = mix(under, over, selector);
    return vec4<f32>(result, srgb.a);
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let color = in.v_Color * textureSample(u_Texture, u_Sampler, in.v_UV);
    return FragmentOutput(color);
}
