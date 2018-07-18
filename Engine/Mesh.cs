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

		public Mesh(Material material, float[] vdata, uint[] indices, Matrix4x4[] modelMatrices) {
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

		public void Draw(Matrix4x4 projView, bool forward) {
			if(forward && Material.Deferred || !forward && !Material.Deferred) return;
			Material.Use(projView);

			GL.BindVertexArray(Vao);
			GL.DrawElementsInstanced(PrimitiveType.Triangles, ElementCount, DrawElementsType.UnsignedInt, IntPtr.Zero, InstanceCount);
		}
	}
}