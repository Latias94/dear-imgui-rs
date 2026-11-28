#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec4 oColor;
layout(location = 1) in vec2 oUV;

layout(binding = 0, set = 0) uniform texture2D imguiTexture;
layout(binding = 0, set = 1) uniform sampler imguiSampler;

layout(push_constant) uniform PushConstants {
    mat4 ortho;
    vec4 gamma_pad; // A negative value selects the sRGB transfer for vertex RGB.
} pc;

layout(location = 0) out vec4 finalColor;

void main() {
    vec4 texel = texture(sampler2D(imguiTexture, imguiSampler), oUV);
    vec3 rgb = max(oColor.rgb, vec3(0.0));
    float gamma = pc.gamma_pad.x;
    if (gamma < 0.0) {
        vec3 low = rgb / 12.92;
        vec3 high = pow((rgb + 0.055) / 1.055, vec3(2.4));
        rgb = mix(low, high, greaterThan(rgb, vec3(0.04045)));
    } else if (gamma != 1.0) {
        rgb = pow(rgb, vec3(gamma));
    }
    finalColor = vec4(rgb, oColor.a) * texel;
}
