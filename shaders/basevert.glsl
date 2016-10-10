#version 410
uniform mat4 MVPMatrix;
uniform mat4 MVMatrix;
in vec3 vPosition;
in vec3 vNormal;
in vec2 vTexCoord;
out vec3 normal;
out vec2 texcoord;
out vec3 pos;
void main() {
    gl_Position = MVPMatrix * vec4(vPosition, 1.);
    pos = (MVMatrix * vec4(vPosition, 1.)).xyz;
    normal = normalize(MVMatrix * vec4(vNormal, 0.)).xyz;
    texcoord = vTexCoord;
}