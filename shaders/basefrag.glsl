#version 410
in vec3 normal;
in vec2 texcoord;
in vec3 pos;
out vec4 outputColor;
uniform mat4 MVMatrix;
uniform sampler2D tex;

// iq method
/*vec4 getTexel(vec2 p) {
    vec2 resolution = textureSize(tex, 0);
    p = p*resolution + 0.5;

    vec2 i = floor(p);
    vec2 f = p - i;
    f = f*f*f*(f*(f*6.0-15.0)+10.0);
    p = i + f;

    p = (p - 0.5)/resolution;
    return texture(tex, p);
}*/

void main() {
    vec3 lightpos = (MVMatrix * vec4(500., 500., 1000., 1.)).xyz;
    vec3 L = normalize(lightpos - pos);
    float diff = dot(normal, L);
    diff = abs(diff) * mix(0.3, 1., sign(diff) / 2. + .5);
    float amb = .2;

    vec4 tcol = texture(tex, texcoord);
    outputColor = vec4(tcol.rgb * clamp(diff + amb, 0.0, 1.0), tcol.a);
    if(tcol.a < .1)
        discard;
}