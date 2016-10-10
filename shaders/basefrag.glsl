#version 410
in vec3 normal;
in vec2 texcoord;
in vec3 pos;
out vec4 outputColor;
uniform mat4 MVMatrix;
uniform sampler2D tex;
void main() {
    vec3 lightpos = (MVMatrix * vec4(500., 500., 1000., 1.)).xyz;
    vec3 L = normalize(lightpos - pos);
    float diff = dot(normal, L);
    diff = abs(diff) * mix(0.3, 1., sign(diff) / 2. + .5);
    float amb = 0;

    vec4 tcol = textureLod(tex, texcoord, textureQueryLod(tex, texcoord).x);
    outputColor = vec4(tcol.rgb * clamp(diff + amb, 0.0, 1.0), tcol.a);
    if(tcol.a < .1)
        discard;
}