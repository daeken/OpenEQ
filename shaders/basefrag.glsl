#version 410
in vec3 normal;
in vec2 texcoord;
out vec4 outputColor;
uniform sampler2D tex;
void main() {
    vec3 tcol = textureLod(tex, texcoord, textureQueryLod(tex, texcoord).x).rgb;
    outputColor = vec4(tcol, 1.0);
}