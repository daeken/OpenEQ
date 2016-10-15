uniform mat4 PMatrix;
uniform mat4 MVMatrix;
uniform float framepos;
attribute vec3 vPosition;
attribute vec3 vNormal;
attribute vec3 vNextPosition;
attribute vec3 vNextNormal;
varying vec3 normal;
void main() {
    gl_Position = PMatrix * MVMatrix * vec4(mix(vPosition, vNextPosition, framepos), 1.);
    normal = normalize(MVMatrix * vec4(mix(vNormal, vNextNormal, framepos), 0.)).xyz;
}