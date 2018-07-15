using System.Collections.Generic;
using System.IO;
using System.Text;
using System.Linq;
using System.Numerics;

namespace OpenEQ.Common {
	public static class Extensions {
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

		public static Dictionary<KeyT, ValueT> ToDictionary<KeyT, ValueT>(this IEnumerable<(KeyT Key, ValueT Value)> e) =>
			e.ToDictionary(x => x.Key, x => x.Value);
		
		public static Vector2 XY(this Vector3 v) => new Vector2(v.X, v.Y);
		public static Vector2 XY(this Vector4 v) => new Vector2(v.X, v.Y);
		
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
	}
}