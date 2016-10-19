#version 410
uniform mat4 MVPMatrix;
uniform mat4 MVMatrix;
in vec3 vPosition;
in vec3 vNormal;
in vec2 vTexCoord;
in vec3 vNewPosition;
in vec3 vNewNormal;
uniform float framepos;
out vec3 normal;
out vec2 texcoord;
out vec3 pos;
void main() {
	vec4 tpos = vec4(mix(vPosition, vNewPosition, framepos), 1.);
    gl_Position = MVPMatrix * tpos;
    pos = (MVMatrix * tpos).xyz;
    normal = normalize(MVMatrix * vec4(mix(vNormal, vNewNormal, framepos), 0.)).xyz;
    texcoord =vTexCoord;
}