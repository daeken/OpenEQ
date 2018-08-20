﻿using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Numerics;
using ImageLib;
using Jitter.Collision;
using Jitter.Dynamics;
using Jitter.LinearMath;
using OpenTK.Graphics.OpenGL4;
using OpenEQ.Common;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class Mesh {
		public Material Material;
		readonly int Vao;
		readonly Buffer<uint> IndexBuffer;
		readonly Buffer<float> VertexBuffer;
		readonly Buffer<Matrix4x4> ModelMatrixBuffer;
		public readonly Matrix4x4[] ModelMatrices;
		public readonly (List<Vector3> Positions, List<TriangleVertexIndices> Triangles) PhysicsMesh;
		public readonly bool IsCollidable;

		public Mesh(Material material, float[] vdata, uint[] indices, Matrix4x4[] modelMatrices, bool isCollidable) {
			Material = material;
			
			GL.BindVertexArray(Vao = GL.GenVertexArray());
			IndexBuffer = new Buffer<uint>(indices, BufferTarget.ElementArrayBuffer);
			VertexBuffer = new Buffer<float>(vdata);
			var pp = 0; // aPosition
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, 0);
			pp = 1; // aNormal
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, 3 * 4);
			pp = 2; // aTexCoord
			GL.EnableVertexAttribArray(pp);
			GL.VertexAttribPointer(pp, 2, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, (3 + 3) * 4);
			
			ModelMatrixBuffer = new Buffer<Matrix4x4>(modelMatrices);
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

			ModelMatrices = modelMatrices;

			IsCollidable = isCollidable;

			if(isCollidable)
				PhysicsMesh = (
					(vdata.Length / 8).Times(i => {
						var offset = i * 8;
						return new Vector3(vdata[offset + 0], vdata[offset + 1], vdata[offset + 2]);
					}).ToList(), 
					(indices.Length / 3).Times(i => {
						var offset = i * 3;
						return new TriangleVertexIndices((int) indices[offset + 0], (int) indices[offset + 1], (int) indices[offset + 2]);
					}).ToList()
				);
		}

		public void Draw(Matrix4x4 projView, bool forward) {
			if(forward && Material.Deferred || !forward && !Material.Deferred) return;
			Material.Use(projView, MaterialUse.Static);

			GL.BindVertexArray(Vao);
			GL.DrawElementsInstanced(PrimitiveType.Triangles, IndexBuffer.Length, DrawElementsType.UnsignedInt, IntPtr.Zero, ModelMatrixBuffer.Length);
		}
	}
}