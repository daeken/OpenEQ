using System.IO;
using System.Text;

namespace OpenEQ.Common {
	public static class Extensions {
		internal static void WriteBool(this BinaryWriter bw, bool value) => bw.Write(value ? 1U : 0U);
		internal static bool ReadBool(this BinaryReader br) => br.ReadUInt32() != 0;

		internal static void WriteString(this BinaryWriter bw, string value) {
			bw.Write(value.Length);
			bw.Write(Encoding.UTF8.GetBytes(value));
		}
		internal static string ReadString(this BinaryReader br) => Encoding.UTF8.GetString(br.ReadBytes(br.ReadInt32()));
		
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
	}
}