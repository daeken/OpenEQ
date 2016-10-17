#version 410
uniform sampler2D Tex;
in vec2 Frag_UV;
in vec4 Frag_Color;
out vec4 Out_Color;
void main() {
    Out_Color = Frag_Color * texture(Tex, Frag_UV.st);
}