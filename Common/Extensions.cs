using System.Collections.Generic;
using System.IO;
using System.Text;
using System.Linq;

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

		public static Vec2 ReadVec2(this BinaryReader br) => new Vec2(br.ReadSingle(), br.ReadSingle());
		public static Vec3 ReadVec3(this BinaryReader br) => new Vec3(br.ReadSingle(), br.ReadSingle(), br.ReadSingle());
		public static Vec4 ReadVec4(this BinaryReader br) => new Vec4(br.ReadSingle(), br.ReadSingle(), br.ReadSingle(), br.ReadSingle());

		internal static void Write(this BinaryWriter bw, Vec2 v) {
			bw.Write((float) v.X);
			bw.Write((float) v.Y);
		}

		internal static void Write(this BinaryWriter bw, Vec3 v) {
			bw.Write((float) v.X);
			bw.Write((float) v.Y);
			bw.Write((float) v.Z);
		}

		internal static void Write(this BinaryWriter bw, Vec4 v) {
			bw.Write((float) v.X);
			bw.Write((float) v.Y);
			bw.Write((float) v.Z);
			bw.Write((float) v.W);
		}

		public static Dictionary<KeyT, ValueT> ToDictionary<KeyT, ValueT>(this IEnumerable<(KeyT Key, ValueT Value)> e) =>
			e.ToDictionary(x => x.Key, x => x.Value);
	}
}