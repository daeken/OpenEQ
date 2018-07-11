using System;
using System.Collections.Generic;
using System.Linq;
using OpenTK.Graphics.OpenGL4;
using OpenEQ.Common;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class Mesh {
		public Material Material;
		readonly int Vao, Pbo, Ibo, Mbo;
		readonly int ElementCount, InstanceCount;

		static Program DeferredProgram, ForwardProgram;

		public Mesh(Material material, float[] vdata, uint[] indices, Mat4[] modelMatrices) {
			if(DeferredProgram == null) {
				DeferredProgram = new Program(@"
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
layout (location = 0) out vec4 color;
layout (location = 1) out vec3 position;
uniform sampler2D uTex;
uniform bool uMasked;
void main() {
	color = texture(uTex, vTexCoord);
	if(uMasked && color.a < 0.5)
		discard;
	color.a = 0;
	position = vPosition;
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
uniform sampler2D uTex;
uniform bool uMasked;
void main() {
	color = texture(uTex, vTexCoord);
	if(uMasked)
		color.a *= (color.r + color.g + color.b) / 3;
}
				");
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
				modelMatrices.Select(x => x.AsArray).SelectMany(x => x).Select(x => (float) x).ToArray(), BufferUsageHint.StaticDraw);
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

		public static void SetProjectionView(Mat4 mat) {
			GL.ActiveTexture(TextureUnit.Texture0);
			DeferredProgram.Use();
			DeferredProgram.SetUniform("uProjectionViewMat", mat);
			DeferredProgram.SetUniform("uTex", 0);
			ForwardProgram.Use();
			ForwardProgram.SetUniform("uProjectionViewMat", mat);
			ForwardProgram.SetUniform("uTex", 0);
		}

		public void Draw(bool translucent) {
			var cp = translucent ? ForwardProgram : DeferredProgram;
			if(translucent && !Material.Flags.HasFlag(MaterialFlag.Translucent))
				return;
			if(!translucent && Material.Flags.HasFlag(MaterialFlag.Translucent))
				return;
			
			cp.Use();
			Material.Use();
			switch(Material.Flags) {
				case MaterialFlag x when x.HasFlag(MaterialFlag.Masked):
					cp.SetUniform("uMasked", 1);
					break;
				case MaterialFlag.Transparent:
					return;
				default:
					cp.SetUniform("uMasked", 0);
					break;
			}

			GL.BindVertexArray(Vao);
			GL.DrawElementsInstanced(PrimitiveType.Triangles, ElementCount, DrawElementsType.UnsignedInt, IntPtr.Zero, InstanceCount);
		}
	}
}