using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Numerics;
using ImageLib;
using OpenTK.Graphics.OpenGL4;
using OpenEQ.Common;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class Mesh {
		public Material Material;
		readonly int Vao, Pbo, Ibo, Mbo;
		readonly int ElementCount, InstanceCount;

		static Program DeferredProgram, ForwardProgram, FireProgram;
		static Texture FlameTexture;

		public Mesh(Material material, float[] vdata, uint[] indices, Matrix4x4[] modelMatrices) {
			if(DeferredProgram == null) {
				DeferredProgram = new Program(@"
#version 410
precision highp float;
layout (location = 0) in vec4 aPosition;
layout (location = 1) in vec3 aNormal;
layout (location = 2) in vec2 aTexCoord;
layout (location = 3) in mat4 aModelMat;
uniform mat4 uProjectionViewMat;
out vec2 vTexCoord;
void main() {
	gl_Position = uProjectionViewMat * aModelMat * aPosition;
	vTexCoord = aTexCoord;
}
			", @"
#version 410
precision highp float;
in vec3 vPosition;
in vec2 vTexCoord;
layout (location = 0) out vec4 color;
uniform sampler2D uTex;
uniform bool uMasked;
void main() {
	color = texture(uTex, vTexCoord);
	if(uMasked && color.a < 0.5)
		discard;
	color.a = 0;
}
				");
				ForwardProgram = new Program(@"
#version 410
precision highp float;
layout (location = 0) in vec4 aPosition;
layout (location = 1) in vec3 aNormal;
layout (location = 2) in vec2 aTexCoord;
layout (location = 3) in mat4 aModelMat;
uniform mat4 uProjectionViewMat;
out vec2 vTexCoord;
void main() {
	gl_Position = uProjectionViewMat * aModelMat * aPosition;
	vTexCoord = aTexCoord;
}
			", @"
#version 410
precision highp float;
in vec3 vPosition;
in vec2 vTexCoord;
out vec4 color;
uniform sampler2D uTex;
uniform bool uMasked;
void main() {
	color = texture(uTex, vTexCoord);
	if(uMasked)
		color.a *= (color.r + color.g + color.b) / 3;
}
				");
				
				// Credit: http://clockworkchilli.com/blog/8_a_fire_shader_in_glsl_for_your_webgl_games
				FireProgram = new Program(@"
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
				var img = Png.Decode(File.OpenRead("flame.png"));
				img.FlipY();
				FlameTexture = new Texture(img, true);
			}

			Material = material;
			
			GL.BindVertexArray(Vao = GL.GenVertexArray());
			GL.BindBuffer(BufferTarget.ElementArrayBuffer, Ibo = GL.GenBuffer());
			GL.BufferData(BufferTarget.ElementArrayBuffer, indices.Length * 4, indices, BufferUsageHint.StaticDraw);
			GL.BindBuffer(BufferTarget.ArrayBuffer, Pbo = GL.GenBuffer());
			GL.BufferData(BufferTarget.ArrayBuffer, vdata.Length * 4, vdata, BufferUsageHint.StaticDraw);
			var pp = 0; // aPosition
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, 0);
			pp = 1; // aNormal
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, 3 * 4);
			pp = 2; // aTexCoord
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 2, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, (3 + 3) * 4);
			
			GL.BindBuffer(BufferTarget.ArrayBuffer, Mbo = GL.GenBuffer());
			GL.BufferData(BufferTarget.ArrayBuffer, modelMatrices.Length * 16 * 4, 
				modelMatrices.Select(x => x.AsArray()).SelectMany(x => x).ToArray(), BufferUsageHint.StaticDraw);
			pp = 3; // aModelMat
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 4, VertexAttribPointerType.Float, false, 4 * 16, 0);
			GL.VertexAttribDivisor(pp, 1);
			GL.EnableVertexAttribArray(pp + 1);
			GL.VertexAttribPointer(pp + 1, 4, VertexAttribPointerType.Float, false, 4 * 16, 4 * 4);
			GL.VertexAttribDivisor(pp + 1, 1);
			GL.EnableVertexAttribArray(pp + 2);
			GL.VertexAttribPointer(pp + 2, 4, VertexAttribPointerType.Float, false, 4 * 16, 8 * 4);
			GL.VertexAttribDivisor(pp + 2, 1);
			GL.EnableVertexAttribArray(pp + 3);
			GL.VertexAttribPointer(pp + 3, 4, VertexAttribPointerType.Float, false, 4 * 16, 12 * 4);
			GL.VertexAttribDivisor(pp + 3, 1);

			ElementCount = indices.Length;
			InstanceCount = modelMatrices.Length;
		}

		public static void SetProjectionView(Matrix4x4 mat) {
			GL.ActiveTexture(TextureUnit.Texture0);
			foreach(var program in new[] { DeferredProgram, ForwardProgram, FireProgram }) {
				program.Use();
				program.SetUniform("uProjectionViewMat", mat);
				program.SetUniform("uTex", 0);
			}
		}

		public void Draw(bool translucent) {
			var cp = translucent ? ForwardProgram : DeferredProgram;
			var setMasked = false;
			if(Material == null) { // TODO: Unhack
				if(!translucent) return;
				FireProgram.Use();
				FireProgram.SetUniform("uTime", Time);
				FlameTexture.Use();
			} else {
				if(Material.Flags == MaterialFlag.Transparent) return;
				
				if(translucent && !Material.Flags.HasFlag(MaterialFlag.Translucent))
					return;
				if(!translucent && Material.Flags.HasFlag(MaterialFlag.Translucent))
					return;

				cp.Use();
				Material.Use();
				if(Material.Flags.HasFlag(MaterialFlag.Masked)) {
					cp.SetUniform("uMasked", 1);
					setMasked = true;
				}
			}

			GL.BindVertexArray(Vao);
			GL.DrawElementsInstanced(PrimitiveType.Triangles, ElementCount, DrawElementsType.UnsignedInt, IntPtr.Zero, InstanceCount);
			
			if(setMasked)
				cp.SetUniform("uMasked", 0);
		}
	}
}