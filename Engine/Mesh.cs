using System;
using System.Collections.Generic;
using System.Linq;
using OpenTK.Graphics.OpenGL4;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class Mesh {
		public Material Material;
		readonly int Vao, Pbo, Ibo, Mbo;
		readonly int ElementCount, InstanceCount;

		static Program Program;

		public Mesh(Material material, float[] vdata, uint[] indices, Mat4[] modelMatrices) {
			if(Program == null)
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
			
			Material = material;
			
			Program.Use();

			GL.BindVertexArray(Vao = GL.GenVertexArray());
			GL.BindBuffer(BufferTarget.ElementArrayBuffer, Ibo = GL.GenBuffer());
			GL.BufferData(BufferTarget.ElementArrayBuffer, indices.Length * 4, indices, BufferUsageHint.StaticDraw);
			GL.BindBuffer(BufferTarget.ArrayBuffer, Pbo = GL.GenBuffer());
			GL.BufferData(BufferTarget.ArrayBuffer, vdata.Length * 4, vdata, BufferUsageHint.StaticDraw);
			var pp = Program.GetAttribute("aPosition");
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, 0);
			pp = Program.GetAttribute("aNormal");
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, 3 * 4);
			pp = Program.GetAttribute("aTexCoord");
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 2, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, (3 + 3) * 4);
			
			GL.BindBuffer(BufferTarget.ArrayBuffer, Mbo = GL.GenBuffer());
			GL.BufferData(BufferTarget.ArrayBuffer, modelMatrices.Length * 16 * 4, 
				modelMatrices.Select(x => x.AsArray).SelectMany(x => x).Select(x => (float) x).ToArray(), BufferUsageHint.StaticDraw);
			pp = Program.GetAttribute("aModelMat");
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
			Program.Use();
			Program.SetUniform("uProjectionViewMat", mat);
			GL.ActiveTexture(TextureUnit.Texture0);
			Program.SetUniform("uTex", 0);
		}

		public void Draw() {
			Material.Use();
			switch(Material.Flags) {
				case MaterialFlag x when x.HasFlag(MaterialFlag.Masked):
					Program.SetUniform("uMasked", 1);
					break;
				case MaterialFlag.Transparent:
					return;
				default:
					Program.SetUniform("uMasked", 0);
					break;
			}

			GL.BindVertexArray(Vao);
			GL.DrawElementsInstanced(PrimitiveType.Triangles, ElementCount, DrawElementsType.UnsignedInt, IntPtr.Zero, InstanceCount);
		}

		public static Mesh Sphere(Material mat) {
			var vertices = new List<Vec3>();
			var normals = new List<Vec3>();
			var index = 0;
			var grid = new List<List<int>>();
			for(var iy = 0; iy <= 32; ++iy) {
				var vrow = new List<int>();
				var v = iy / 32.0;
				for(var ix = 0; ix <= 32; ++ix) {
					var u = ix / 32.0;

					var vert = vec3(
						-Math.Cos(u * Math.PI * 2) * Math.Sin(v * Math.PI), 
						Math.Cos(v * Math.PI), 
						Math.Sin(u * Math.PI * 2) * Math.Sin(v * Math.PI)
					);
					
					vertices.Add(vert);
					normals.Add(vert.Normalized);
					vrow.Add(index++);
				}
				grid.Add(vrow);
			}

			var indices = new List<(int, int, int)>();

			for(var iy = 0; iy < 32; ++iy) {
				for(var ix = 0; ix < 32; ++ix) {
					var a = grid[iy][ix + 1];
					var b = grid[iy][ix];
					var c = grid[iy + 1][ix];
					var d = grid[iy + 1][ix + 1];

					if(iy > 0) indices.Add((a, b, d));
					if(iy < 31) indices.Add((b, c, d));
				}
			}
			
			var buf = new List<double>();
			for(var i = 0; i < vertices.Count; ++i) {
				buf.Add(vertices[i].X);
				buf.Add(vertices[i].Y);
				buf.Add(vertices[i].Z);
				buf.Add(normals[i].X);
				buf.Add(normals[i].Y);
				buf.Add(normals[i].Z);
				// UV
				buf.Add(0.5);
				buf.Add(0.5);
			}
			
			return new Mesh(mat, 
				buf.Select(x => (float) x).ToArray(), 
				indices.Select(x => new[] { (uint) x.Item1, (uint) x.Item2, (uint) x.Item3 }).SelectMany(x => x).ToArray(), 
				new[] { Mat4.Identity }
			);
		}
	}
}