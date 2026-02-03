#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec4 oColor;
layout(location = 1) in vec2 oUV;

layout(binding = 0, set = 0) uniform sampler2D fontsSampler;

layout(push_constant) uniform PushConstants {
    mat4 ortho;
    vec4 gamma_pad; // gamma in .x
} pc;

layout(location = 0) out vec4 finalColor;

void main() {
    vec4 color = oColor * texture(fontsSampler, oUV);
    float gamma = pc.gamma_pad.x;
    vec3 corrected = pow(color.rgb, vec3(gamma));
    finalColor = vec4(corrected, color.a);
}

