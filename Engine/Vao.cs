using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Numerics;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
	public class Vao {
		readonly int Id;
		bool Destroyed;
		
		static int Current;
		
		readonly List<object> AttachedBuffers = new List<object>(); // Prevent garbage collection
		
		public Vao() => Id = GL.GenVertexArray();
		~Vao() => Destroy();

		public void Destroy() {
			if(Destroyed) return;
			Destroyed = true;
			Unbind();
			GL.DeleteVertexArray(Id);
		}

		public void Bind() {
			if(Current == Id) return;
			Debug.Assert(!Destroyed);
			GL.BindVertexArray(Id);
			Current = Id;
		}
		
		public void Unbind() {
			if(Current != Id) return;
			GL.BindVertexArray(0);
			Current = 0;
		}

		public void Bind(Action func) {
			var prev = Current;
			Bind();
			func();
			if(Current != prev) {
				GL.BindVertexArray(prev);
				Current = prev;
			}
		}

		void Attach<T>(Buffer<T> buffer, bool instanced, params (int Name, Type Type)[] attributes) where T : struct {
			AttachedBuffers.Add(buffer);
			void Attach(VertexAttribPointerType glType, int size) {
				var stride = 0;
				var offsets = attributes.Select(attr => {
					var offset = stride;
					switch(attr.Type) {
						case var x when x == typeof(Vector2): stride += 2; break;
						case var x when x == typeof(Vector3): stride += 3; break;
						case var x when x == typeof(Vector4): stride += 4; break;
						case var x when x == typeof(Matrix4x4): stride += 16; break;
						default: stride++; break;
					}
					return (attr.Name, offset, stride - offset);
				}).ToList();
				foreach(var (_name, offset, attrSize) in offsets) {
					for(int i = 0, name = _name; i < attrSize; i += 4, ++name) {
						GL.EnableVertexAttribArray(name);
						GL.VertexAttribPointer(name, Math.Min(attrSize, 4), glType, false, stride * size, (offset + i) * size);
						if(instanced)
							GL.VertexAttribDivisor(name, 1);
					}
				}
			}
			
			Bind(() => {
				buffer.Bind();
				if(buffer.Target == BufferTarget.ElementArrayBuffer) return; // Just binding it is enough
				switch(buffer) {
					case Buffer<short> _: Attach(VertexAttribPointerType.Short, 2); break;
					case Buffer<ushort> _: Attach(VertexAttribPointerType.UnsignedShort, 2); break;
					case Buffer<int> _: Attach(VertexAttribPointerType.Int, 4); break;
					case Buffer<uint> _: Attach(VertexAttribPointerType.UnsignedInt, 4); break;
					
					case Buffer<float> _:
					case Buffer<Vector2> _: case Buffer<Vector3> _: case Buffer<Vector4> _:
					case Buffer<Matrix4x4> _: Attach(VertexAttribPointerType.Float, 4); break;
					
					default:
						throw new NotImplementedException($"Unsupported buffer type in Vao.Attach: {typeof(T)}");
				}
			});
		}

		public void Attach(Buffer<byte> buffer, params (int Name, VertexAttribPointerType Type, int Count)[] attributes) {
			AttachedBuffers.Add(buffer);
			Bind(() => {
				buffer.Bind();
				if(buffer.Target == BufferTarget.ElementArrayBuffer) return; // Just binding it is enough
				
				var stride = 0;
				var offsets = attributes.Select(attr => {
					var offset = stride;
					switch(attr.Type) {
						case VertexAttribPointerType.Byte:
						case VertexAttribPointerType.UnsignedByte:
							stride += attr.Count * 1;
							break;
						case VertexAttribPointerType.Float:
						case VertexAttribPointerType.Int:
						case VertexAttribPointerType.UnsignedInt:
							stride += attr.Count * 4;
							break;
						default: throw new NotImplementedException();
					}
					return (attr.Name, attr.Type, offset, attr.Count);
				}).ToList();
				foreach(var (name, glType, offset, count) in offsets) {
					GL.EnableVertexAttribArray(name);
					GL.VertexAttribPointer(name, count, glType, glType == VertexAttribPointerType.UnsignedByte, stride, offset);
				}
			});
		}

		public void Attach<T>(Buffer<T> buffer, params (int Name, Type Type)[] attributes) where T : struct =>
			Attach(buffer, false, attributes);
		
		public void AttachInstanced<T>(Buffer<T> buffer, params (int Name, Type Type)[] attributes) where T : struct =>
			Attach(buffer, true, attributes);
	}
}