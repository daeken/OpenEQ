using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Numerics;
using System.Runtime.InteropServices;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
	public class Buffer<T> where T : struct {
		public readonly int Length;
		public readonly BufferTarget Target;
		readonly int Object;

		bool Destroyed;

		public unsafe Buffer(T[] data, BufferTarget target = BufferTarget.ArrayBuffer, BufferUsageHint usage = BufferUsageHint.StaticDraw) {
			Object = GL.GenBuffer();
			Target = target;
			Length = data.Length;
			Bind();
			switch(data) {
				case byte[] vdata:
					fixed(byte* p = vdata)
						GL.BufferData(target, vdata.Length, (IntPtr) p, usage);
					break;
				case float[] vdata:
					fixed(float* p = vdata)
						GL.BufferData(target, vdata.Length * 4, (IntPtr) p, usage);
					break;
				case uint[] vdata:
					fixed(uint* p = vdata)
						GL.BufferData(target, vdata.Length * 4, (IntPtr) p, usage);
					break;
				case ushort[] vdata:
					fixed(ushort* p = vdata)
						GL.BufferData(target, vdata.Length * 2, (IntPtr) p, usage);
					break;
				case Vector3[] vdata:
					fixed(Vector3* p = vdata)
						GL.BufferData(target, vdata.Length * 3 * 4, (IntPtr) p, usage);
					break;
				case Matrix4x4[] vdata:
					fixed(Matrix4x4* p = vdata)
						GL.BufferData(target, vdata.Length * 16 * 4, (IntPtr) p, usage);
					break;
				default:
					throw new NotSupportedException();
			}
		}

		~Buffer() => Destroy();

		public void Destroy() {
			if(Destroyed) return;
			Destroyed = true;
			GL.DeleteBuffer(Object);
		}

		public void Bind() {
			Debug.Assert(!Destroyed);
			GL.BindBuffer(Target, Object);
		}
	}
}