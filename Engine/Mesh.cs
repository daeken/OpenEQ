using System;
using System.Collections.Generic;
using System.Linq;
using OpenTK.Graphics.OpenGL4;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class Mesh {
		public Material Material;
		public readonly int Vao, Pbo, Ibo;
		public readonly int ElementCount;

		static Program Program;

		public Mesh(Material material, float[] vdata, uint[] indices) {
			if(Program == null)
				Program = new Program(@"
#version 410
precision highp float;
layout (location = 0) in vec4 aPosition;
layout (location = 1) in vec3 aNormal;
layout (location = 2) in vec2 aTexCoord;
layout (location = 3) in vec4 aColor;
uniform mat4 uProjectionViewMat, uModelMat;
out vec3 vNormal;
out vec2 vTexCoord;
out vec4 vColor;
void main() {
	gl_Position = uProjectionViewMat * uModelMat * aPosition;
	vNormal = aNormal;
	vTexCoord = aTexCoord;
	vColor = aColor;
}
			", @"
#version 410
precision highp float;
in vec3 vNormal;
in vec2 vTexCoord;
in vec4 vColor;
layout (location = 0) out vec4 color;
uniform sampler2D uTex;
uniform bool uMasked, uFake;
void main() {
	if(uFake) {
		color = vec4(0, 1, 0, .1);
		return;
	}
	color = texture(uTex, vTexCoord);
	if(uMasked && color.a < 0.5)
		discard;
	color = vec4(color.rgb + vColor.bgr * (vColor.a - .5) - 0.1, color.a);
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
			GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, (3 + 3 + 2 + 1) * 4, 0);
			pp = Program.GetAttribute("aNormal");
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, (3 + 3 + 2 + 1) * 4, 3 * 4);
			pp = Program.GetAttribute("aTexCoord");
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 2, VertexAttribPointerType.Float, false, (3 + 3 + 2 + 1) * 4, (3 + 3) * 4);
			pp = Program.GetAttribute("aColor");
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 4, VertexAttribPointerType.UnsignedByte, true, (3 + 3 + 2 + 1) * 4, (3 + 3 + 2) * 4);

			ElementCount = indices.Length;
		}

		public static void SetProjectionView(Mat4 mat) {
			Program.Use();
			Program.SetUniform("uProjectionViewMat", mat);
			GL.ActiveTexture(TextureUnit.Texture0);
			Program.SetUniform("uTex", 0);
		}

		public void Draw(Mat4 modelMat) {
			Program.SetUniform("uModelMat", modelMat);
			if(Material != null) {
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
			} else {
				Program.SetUniform("uFake", 1);
				GL.Enable(EnableCap.Blend);
				GL.BlendFunc(BlendingFactor.SrcAlpha, BlendingFactor.OneMinusSrcAlpha);
				GL.DepthMask(false);
				GL.Disable(EnableCap.CullFace);
			}

			GL.BindVertexArray(Vao);
			GL.DrawElements(PrimitiveType.Triangles, ElementCount, DrawElementsType.UnsignedInt, 0);
			
			if(Material == null) {
				Program.SetUniform("uFake", 0);
				GL.Disable(EnableCap.Blend);
				GL.DepthMask(true);
				GL.Enable(EnableCap.CullFace);
			}
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
				// Color
				buf.Add(0.5);
			}
			
			return new Mesh(mat, 
				buf.Select(x => (float) x).ToArray(), 
				indices.Select(x => new[] { (uint) x.Item1, (uint) x.Item2, (uint) x.Item3 }).SelectMany(x => x).ToArray());
		}
	}
}