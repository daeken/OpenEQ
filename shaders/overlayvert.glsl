#version 410
uniform mat4 Projection;
uniform vec2 Translation;
in vec2 Position;
in vec2 UV;
in vec4 Color;
out vec2 Frag_UV;
out vec4 Frag_Color;
void main() {
   Frag_UV = UV;
   Frag_Color = Color;
   gl_Position = Projection * vec4(Translation + Position, 0, 1);
}