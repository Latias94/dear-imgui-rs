struct Parameters {
    width: u32,
    height: u32,
    layer: u32,
    mip: u32,
    exposure: f32,
    depth_min: f32,
    depth_max: f32,
    padding: u32,
}

@group(0) @binding(0) var<uniform> parameters: Parameters;
@group(0) @binding(1) var ids: texture_storage_2d<r32uint, write>;
@group(0) @binding(2) var hdr: texture_storage_2d<rgba16float, write>;

@vertex
fn fullscreen(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(positions[index], 0.0, 1.0);
}

@fragment
fn depth_ramp(@builtin(position) position: vec4<f32>) -> @builtin(frag_depth) f32 {
    let x = floor(position.x) / f32(max(parameters.width - 1u, 1u));
    return 0.5 * x + 0.125 * f32(parameters.layer) + 0.0625 * f32(parameters.mip);
}

@compute @workgroup_size(8, 8)
fn produce(@builtin(global_invocation_id) invocation: vec3<u32>) {
    if invocation.x >= parameters.width || invocation.y >= parameters.height {
        return;
    }
    let pixel = vec2<i32>(invocation.xy);
    let dimensions = vec2<u32>(max(parameters.width - 1u, 1u), max(parameters.height - 1u, 1u));
    let uv = vec2<f32>(invocation.xy) / vec2<f32>(dimensions);
    let id = ((invocation.x / 8u) + (invocation.y / 8u) * 3u +
        parameters.layer * 5u + parameters.mip * 7u) % 8u;
    textureStore(ids, pixel, vec4<u32>(id, 0u, 0u, 0u));
    textureStore(hdr, pixel, vec4<f32>(
        4.0 * uv.x + f32(parameters.layer),
        2.0 * uv.y + f32(parameters.mip),
        0.5 + 0.5 * f32(parameters.layer), 1.0));
}
