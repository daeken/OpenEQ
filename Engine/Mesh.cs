using System;
using System.Collections.Generic;
using OpenTK.Graphics.OpenGL4;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class Mesh {
		public readonly Material Material;
		public readonly int Vao, Pbo, Nbo, Tbo, Ibo;
		public readonly int ElementCount;

		static Program Program;

		public Mesh(Material material, float[] positions, float[] normals, float[] texcoords, uint[] indices) {
			if(Program == null)
				Program = new Program(@"
#version 410
precision highp float;
layout (location = 0) in vec4 aPosition;
layout (location = 1) in vec3 aNormal;
layout (location = 2) in vec2 aTexCoord;
uniform mat4 uProjectionMat, uViewMat, uModelMat;
out vec3 vNormal;
out vec2 vTexCoord;
void main() {
	gl_Position = uProjectionMat * uViewMat * uModelMat * aPosition;
	vNormal = aNormal;
	vTexCoord = aTexCoord;
}
			", @"
#version 410
precision highp float;
in vec3 vNormal;
in vec2 vTexCoord;
layout (location = 0) out vec4 color;
uniform sampler2D uTex;
uniform bool uMasked;
void main() {
	color = texture(uTex, vTexCoord);
	if(uMasked && color.a < 0.5)
		discard;
}
				");
			
			Material = material;
			
			Program.Use();

			GL.BindVertexArray(Vao = GL.GenVertexArray());
			GL.BindBuffer(BufferTarget.ElementArrayBuffer, Ibo = GL.GenBuffer());
			GL.BufferData(BufferTarget.ElementArrayBuffer, indices.Length * 4, indices, BufferUsageHint.StaticDraw);
			GL.BindBuffer(BufferTarget.ArrayBuffer, Pbo = GL.GenBuffer());
			GL.BufferData(BufferTarget.ArrayBuffer, positions.Length * 4, positions, BufferUsageHint.StaticDraw);
			var pp = Program.GetAttribute("aPosition");
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, 0, 0);
			GL.BindBuffer(BufferTarget.ArrayBuffer, Nbo = GL.GenBuffer());
			GL.BufferData(BufferTarget.ArrayBuffer, normals.Length * 4, normals, BufferUsageHint.StaticDraw);
			pp = Program.GetAttribute("aNormal");
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, 0, 0);
			GL.BindBuffer(BufferTarget.ArrayBuffer, Tbo = GL.GenBuffer());
			GL.BufferData(BufferTarget.ArrayBuffer, texcoords.Length * 4, texcoords, BufferUsageHint.StaticDraw);
			pp = Program.GetAttribute("aTexCoord");
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 2, VertexAttribPointerType.Float, false, 0, 0);

			ElementCount = indices.Length;
		}

		public void Draw(Mat4 modelMat) {
			Program.Use();
			Program.SetUniform("uProjectionMat", ProjectionMat);
			Program.SetUniform("uViewMat", FpsCamera.Matrix);
			Program.SetUniform("uModelMat", modelMat);
			Program.SetUniform("uTex", 0);
			GL.ActiveTexture(TextureUnit.Texture0);
			Material.Use();
			switch(Material.Flags) {
				case MaterialFlag.Masked:
					Program.SetUniform("uMasked", 1);
					break;
				case MaterialFlag.Transparent:
					return;
				default:
					Program.SetUniform("uMasked", 0);
					break;
			}
			GL.BindVertexArray(Vao);
			GL.DrawElements(PrimitiveType.Triangles, ElementCount, DrawElementsType.UnsignedInt, 0);
		}
	}
}