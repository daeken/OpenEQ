using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Numerics;
using System.Text;
using MoreLinq.Extensions;

namespace OpenEQ.Common {
	public static class Extensions {
		public static string Stringify<T>(this T[] arr) => arr == null ? "null" : $"[{string.Join(", ", arr.Select(x => x.ToString()))}]";
		
		internal static void WriteBool(this BinaryWriter bw, bool value) => bw.Write(value ? 1U : 0U);
		internal static bool ReadBool(this BinaryReader br) => br.ReadUInt32() != 0;

		internal static void WriteUTF8String(this BinaryWriter bw, string value) {
			var bytes = Encoding.UTF8.GetBytes(value);
			bw.Write(bytes.Length);
			bw.Write(bytes);
		}
		internal static string ReadUTF8String(this BinaryReader br) => Encoding.UTF8.GetString(br.ReadBytes(br.ReadInt32()));

		public static Vector2 ReadVec2(this BinaryReader br) => new Vector2(br.ReadSingle(), br.ReadSingle());
		public static Vector3 ReadVec3(this BinaryReader br) => new Vector3(br.ReadSingle(), br.ReadSingle(), br.ReadSingle());
		public static Vector4 ReadVec4(this BinaryReader br) => new Vector4(br.ReadSingle(), br.ReadSingle(), br.ReadSingle(), br.ReadSingle());
		public static Quaternion ReadQuaternion(this BinaryReader br) => new Quaternion(br.ReadSingle(), br.ReadSingle(), br.ReadSingle(), br.ReadSingle());

		internal static void Write(this BinaryWriter bw, Vector2 v) {
			bw.Write(v.X);
			bw.Write(v.Y);
		}

		internal static void Write(this BinaryWriter bw, Vector3 v) {
			bw.Write(v.X);
			bw.Write(v.Y);
			bw.Write(v.Z);
		}

		internal static void Write(this BinaryWriter bw, Vector4 v) {
			bw.Write(v.X);
			bw.Write(v.Y);
			bw.Write(v.Z);
			bw.Write(v.W);
		}

		internal static void Write(this BinaryWriter bw, Quaternion v) {
			bw.Write(v.X);
			bw.Write(v.Y);
			bw.Write(v.Z);
			bw.Write(v.W);
		}

		public static Vector2 XY(this Vector3 v) => new Vector2(v.X, v.Y);
		public static Vector2 XY(this Vector4 v) => new Vector2(v.X, v.Y);
		
		public static Vector3 XZY(this Vector3 v) => new Vector3(v.X, v.Z, v.Y);
		public static Vector3 YXZ(this Vector3 v) => new Vector3(v.Y, v.X, v.Z);
		public static Vector3 ZYX(this Vector3 v) => new Vector3(v.Z, v.Y, v.X);
		
		public static Vector2 Add(this Vector2 v, float right) => new Vector2(v.X + right, v.Y + right);

		public static Vector3 Cross(this Vector3 left, Vector3 right) => Vector3.Cross(left, right);
		public static Vector3 Normalized(this Vector3 v) => Vector3.Normalize(v);

		public static float[] AsArray(this Vector2 v) => new[] { v.X, v.Y };
		public static float[] AsArray(this Vector3 v) => new[] { v.X, v.Y, v.Z };
		public static float[] AsArray(this Vector4 v) => new[] { v.X, v.Y, v.Z, v.W };
		
		public static float[] AsArray(this Matrix4x4 mat) => new[] {
			mat.M11, mat.M12, mat.M13, mat.M14, 
			mat.M21, mat.M22, mat.M23, mat.M24, 
			mat.M31, mat.M32, mat.M33, mat.M34, 
			mat.M41, mat.M42, mat.M43, mat.M44 
		};

		public static IEnumerable<int> Range(this int count) => Enumerable.Range(0, count);
		public static IEnumerable<int> Range(this (int Start, int End) tuple) =>
			Enumerable.Range(tuple.Start, tuple.End - tuple.Start);
		public static IEnumerable<int> Range(this (int Start, int End, int Step) tuple) {
			for(var i = tuple.Start; i < tuple.End; i += tuple.Step)
				yield return i;
		}
		
		public static IEnumerable<int> Times(this int count) => Enumerable.Range(0, count);
		public static IEnumerable<int> Times(this uint count) => ((int) count).Times();
		public static IEnumerable<int> Times(this ushort count) => ((int) count).Times();

		public static IEnumerable<T> Times<T>(this int count, Func<T> func) => count.Times().Select(x => func());
		public static IEnumerable<T> Times<T>(this uint count, Func<T> func) => ((int) count).Times(func);
		public static IEnumerable<T> Times<T>(this ushort count, Func<T> func) => ((int) count).Times(func);

		public static IEnumerable<T> Times<T>(this int count, Func<int, T> func) => count.Times().Select(func);
		public static IEnumerable<T> Times<T>(this uint count, Func<int, T> func) => count.Times().Select(func);
		public static IEnumerable<T> Times<T>(this ushort count, Func<int, T> func) => count.Times().Select(func);

		public static void Times(this int count, Action func) => count.Times().ForEach(_ => func());
		public static void Times(this uint count, Action func) => count.Times().ForEach(_ => func());
		public static void Times(this ushort count, Action func) => count.Times().ForEach(_ => func());

		public static void Times(this int count, Action<int> func) => count.Times().ForEach(func);
		public static void Times(this uint count, Action<int> func) => count.Times().ForEach(func);
		public static void Times(this ushort count, Action<int> func) => count.Times().ForEach(func);
	}
}