using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Numerics;
using CollisionManager;
using ImageLib;
using OpenTK.Graphics.OpenGL4;
using OpenEQ.Common;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class Mesh {
		public Material Material;
		readonly Vao Vao;
		readonly Buffer<uint> IndexBuffer;
		readonly Buffer<float> VertexBuffer;
		readonly Buffer<Matrix4x4> ModelMatrixBuffer;
		public readonly Matrix4x4[] ModelMatrices;
		public readonly List<Triangle> PhysicsMesh;
		public readonly bool IsCollidable;

		public Mesh(Material material, float[] vdata, uint[] indices, Matrix4x4[] modelMatrices, bool isCollidable) {
			Material = material;
			
			Vao = new Vao();
			Vao.Attach(IndexBuffer = new Buffer<uint>(indices, BufferTarget.ElementArrayBuffer));
			Vao.Attach(VertexBuffer = new Buffer<float>(vdata), (0, typeof(Vector3)), (1, typeof(Vector3)), (2, typeof(Vector2)));
			Vao.AttachInstanced(ModelMatrixBuffer = new Buffer<Matrix4x4>(modelMatrices), (3, typeof(Matrix4x4)));

			ModelMatrices = modelMatrices;

			IsCollidable = isCollidable;

			if(IsCollidable)
				PhysicsMesh = (indices.Length / 3).Times(i => {
					i *= 3;
					Vector3 GetPoint(int off) {
						var voff = indices[i + off] * 8;
						return vec3(vdata[voff + 0], vdata[voff + 1], vdata[voff + 2]);
					}
	
					return new Triangle(GetPoint(0), GetPoint(1), GetPoint(2));
				}).ToList();
		}

		public void Draw(Matrix4x4 projView, bool forward) {
			if(forward && Material.Deferred || !forward && !Material.Deferred) return;
			Material.Use(projView, MaterialUse.Static);

			Vao.Bind(() => GL.DrawElementsInstanced(PrimitiveType.Triangles, IndexBuffer.Length, DrawElementsType.UnsignedInt, IntPtr.Zero, ModelMatrixBuffer.Length));
		}
	}
}