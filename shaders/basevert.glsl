#version 410
uniform mat4 MVPMatrix;
uniform sampler2D tex;
in vec3 vPosition;
in vec3 vNormal;
in vec2 vTexCoord;
out vec3 normal;
out vec2 texcoord;
void main() {
    gl_Position = MVPMatrix * vec4(vPosition, 1.0);
    normal = vNormal;
    texcoord = vTexCoord / textureSize(tex, 0);
}