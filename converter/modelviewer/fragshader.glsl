precision highp float;

varying vec3 normal;

void main() {
    gl_FragColor = vec4(abs(normal), 1.0);
}