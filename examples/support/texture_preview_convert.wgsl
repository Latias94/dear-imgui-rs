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
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(2) var ids: texture_2d<u32>;
@group(0) @binding(3) var hdr: texture_2d<f32>;
@group(0) @binding(4) var depth_preview: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(5) var id_preview: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(6) var hdr_preview: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn convert(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let output_size = textureDimensions(depth_preview);
    if any(invocation.xy >= output_size) {
        return;
    }
    let output_pixel = vec2<i32>(invocation.xy);
    let source_size = textureDimensions(ids);
    let source_pixel = vec2<i32>(min(invocation.xy * source_size / output_size, source_size - vec2<u32>(1u)));
    let d = textureLoad(depth, source_pixel, 0);
    let mapped_depth = clamp((d - parameters.depth_min) /
        max(parameters.depth_max - parameters.depth_min, 0.0001), 0.0, 1.0);
    textureStore(depth_preview, output_pixel, vec4<f32>(vec3<f32>(mapped_depth), 1.0));

    let id = textureLoad(ids, source_pixel, 0).x;
    let palette = vec3<f32>(f32(id & 1u), f32((id >> 1u) & 1u), f32((id >> 2u) & 1u));
    textureStore(id_preview, output_pixel, vec4<f32>(palette, 1.0));

    let linear = max(textureLoad(hdr, source_pixel, 0).rgb, vec3<f32>(0.0)) * exp2(parameters.exposure);
    textureStore(hdr_preview, output_pixel, vec4<f32>(linear / (vec3<f32>(1.0) + linear), 1.0));
}
