using System.IO;
using System.Linq;
using System.Numerics;
using ImageLib;
using OpenEQ.Engine;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Materials {
	public class FireMaterial : Material {
		public override bool Deferred => false;
		static Program Program;
		static Texture Texture;
		
		public FireMaterial() {
			if(Program == null) {
				Program = new Program(@"
#version 410
precision highp float;
layout (location = 0) in vec4 aPosition;
layout (location = 1) in vec3 aNormal;
layout (location = 2) in vec2 aTexCoord;
layout (location = 3) in mat4 aModelMat;
uniform mat4 uProjectionViewMat;
out vec3 vPosition;
out vec2 vTexCoord;
void main() {
	vec4 pos = aModelMat * aPosition;
	gl_Position = uProjectionViewMat * pos;
	vPosition = pos.xyz;
	vTexCoord = aTexCoord;
}
					", @"
#version 410
precision highp float;
in vec3 vPosition;
in vec2 vTexCoord;
out vec4 color;
uniform float uTime;
uniform sampler2D uTex;

vec4 fire(float time, vec2 tc) {
	// Generate noisy x value
	vec2 n0Uv = vec2(tc.x*1.4 + 0.01, tc.y - time*0.69);
	vec2 n1Uv = vec2(tc.x*0.5 - 0.033, tc.y*2.0 - time*0.12);
	vec2 n2Uv = vec2(tc.x*0.94 + 0.02, tc.y*3.0 - time*0.61);
	float n0 = (texture(uTex, n0Uv).w-0.5)*2.0;
	float n1 = (texture(uTex, n1Uv).w-0.5)*2.0;
	float n2 = (texture(uTex, n2Uv).w-0.5)*2.0;
	float noiseA = clamp(n0 + n1 + n2, -1.0, 1.0);
	
	// Generate noisy y value
	vec2 n0UvB = vec2(tc.x*0.7 - 0.01, tc.y - time*0.27);
	vec2 n1UvB = vec2(tc.x*0.45 + 0.033, tc.y*1.9 - time*0.71);
	vec2 n2UvB = vec2(tc.x*0.8 - 0.02, tc.y*2.5 - time*0.63);
	float n0B = (texture(uTex, n0UvB).w-0.5)*2.0;
	float n1B = (texture(uTex, n1UvB).w-0.5)*2.0;
	float n2B = (texture(uTex, n2UvB).w-0.5)*2.0;
	float noiseB = clamp(n0B + n1B + n2B, -1.0, 1.0);
	
	vec2 finalNoise = vec2(noiseA, noiseB);
	float perturb = (1.0 - tc.y) * 0.35 + 0.02;
	finalNoise = (finalNoise * perturb) + tc - 0.02;
	vec4 ret = texture(uTex, finalNoise);
	ret = vec4(ret.x*2.0, ret.y*0.9, (ret.y/ret.x)*0.2, 1.0);
	finalNoise = clamp(finalNoise, 0.05, 1.0);
	ret.w = texture(uTex, finalNoise).z*2.0;
	ret.w = ret.w*texture(uTex, tc / vec2(1.3, 1.5)).z;
	ret *= vec4(1.0, 1.3, 0.5, 0.6);
	return ret;
}

void main() {
	float time = uTime * 1.3 + length(vPosition);
	vec2 tc = abs(mod(vTexCoord, 1));
	if(tc.y > .95) discard;
	color = (fire(mod(time, 10), tc) + fire(mod(time + .05, 10), tc)) / 2;
}
				");
				Program.SetUniform("uTex", 0);
				var img = Png.Decode(File.OpenRead("flame.png"));
				img.FlipY();
				Texture = new Texture(img, true);
			}
		}

		public override void Use(Matrix4x4 projView) {
			Program.Use();
			Program.SetUniform("uProjectionViewMat", projView);
			Program.SetUniform("uTime", Globals.Time);
			GL.ActiveTexture(TextureUnit.Texture0);
			Texture.Use();
		}
	}
}