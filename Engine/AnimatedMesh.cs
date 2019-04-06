using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using MoreLinq;
using OpenEQ.Common;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
	public class AnimatedMesh {
		public bool Enabled = true;
		
		public readonly Material Material;
		readonly Buffer<uint> IndexBuffer;
		readonly Dictionary<string, (IReadOnlyList<Vao> Vaos, IReadOnlyList<Buffer<float>> Buffers)> Animations;
		
		public AnimatedMesh(Material material, Dictionary<string, IReadOnlyList<IReadOnlyList<float>>> animations, uint[] indices) {
			Material = material;
			IndexBuffer = new Buffer<uint>(indices, BufferTarget.ElementArrayBuffer);

			Animations = animations.Select(kv => {
				var buffers = kv.Value.Select(x => new Buffer<float>(x.ToArray())).ToList();
				var vaos = buffers.Count.Times(i => {
					var vao = new Vao();
					vao.Attach(IndexBuffer);
					vao.Attach(buffers[i], (0, typeof(Vector3)), (1, typeof(Vector3)), (2, typeof(Vector2)));
					vao.Attach(buffers[(i + 1) % buffers.Count], (3, typeof(Vector3)), (4, typeof(Vector3)), (5, typeof(Vector2)));
					return vao;
				}).ToList();
				return (kv.Key, ((IReadOnlyList<Vao>) vaos, (IReadOnlyList<Buffer<float>>) buffers));
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

			Animations[animation].Vaos[frameCount % Animations[animation].Vaos.Count].Bind(() => GL.DrawElements(PrimitiveType.Triangles, IndexBuffer.Length, DrawElementsType.UnsignedInt, IntPtr.Zero));
		}
	}
}