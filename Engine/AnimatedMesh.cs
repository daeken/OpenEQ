using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using OpenEQ.Common;
using MoreLinq;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
	public class AnimatedMesh {
		public bool Enabled = true;
		
		public readonly Material Material;
		readonly Buffer<uint> IndexBuffer;
		readonly Dictionary<string, (IReadOnlyList<int> Vaos, IReadOnlyList<Buffer<float>> Buffers)> Animations;
		
		public AnimatedMesh(Material material, Dictionary<string, IReadOnlyList<IReadOnlyList<float>>> animations, uint[] indices) {
			Material = material;
			IndexBuffer = new Buffer<uint>(indices, BufferTarget.ElementArrayBuffer);

			Animations = animations.Select(kv => {
				var buffers = kv.Value.Select(x => new Buffer<float>(x.ToArray())).ToList();
				var vaos = buffers.Count.Times(i => {
					var vao = GL.GenVertexArray();
					GL.BindVertexArray(vao);
					IndexBuffer.Bind();

					buffers[i].Bind();
					var pp = 0; // aPosition1
					GL.EnableVertexAttribArray(pp);
					GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, 0);
					pp = 1; // aNormal1
					GL.EnableVertexAttribArray(pp);
					GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, 3 * 4);
					pp = 2; // aTexCoord1
					GL.EnableVertexAttribArray(pp);
					GL.VertexAttribPointer(pp, 2, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, (3 + 3) * 4);

					buffers[(i + 1) % buffers.Count].Bind();
					pp = 3; // aPosition2
					GL.EnableVertexAttribArray(pp);
					GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, 0);
					pp = 4; // aNormal2
					GL.EnableVertexAttribArray(pp);
					GL.VertexAttribPointer(pp, 3, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, 3 * 4);
					pp = 5; // aTexCoord2
					GL.EnableVertexAttribArray(pp);
					GL.VertexAttribPointer(pp, 2, VertexAttribPointerType.Float, false, (3 + 3 + 2) * 4, (3 + 3) * 4);
					
					return vao;
				}).ToList();
				return (kv.Key, ((IReadOnlyList<int>) vaos, (IReadOnlyList<Buffer<float>>) buffers));
			}).ToDictionary();
		}

		public void Draw(Matrix4x4 projView, Matrix4x4 modelMat, string animation, float aniTime, bool forward) {
			if(!Enabled) return;
			if(forward && Material.Deferred || !forward && !Material.Deferred) return;
			Material.Use(projView, MaterialUse.Animated);
			Material.SetModelMatrix(modelMat);
			const float fps = 1f / 10;
			var frameCount = (int) (aniTime / fps);
			Material.SetInterpolation(aniTime % fps / fps);

			GL.BindVertexArray(Animations[animation].Vaos[frameCount % Animations[animation].Vaos.Count]);
			GL.DrawElements(PrimitiveType.Triangles, IndexBuffer.Length, DrawElementsType.UnsignedInt, IntPtr.Zero);
		}
	}
}