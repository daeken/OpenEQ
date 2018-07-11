using System.IO;
using System.Linq;
using OpenEQ.Common;

namespace OpenEQ.LegacyFileReader {
	public static class Extensions {
		public static string Stringify<T>(this T[] arr) => $"[{string.Join(", ", arr.Select(x => x.ToString()))}]";
		
		public static Vec2 ReadVec2(this BinaryReader br) => new Vec2(br.ReadSingle(), br.ReadSingle());
		public static Vec3 ReadVec3(this BinaryReader br) => new Vec3(br.ReadSingle(), br.ReadSingle(), br.ReadSingle());
		public static Vec4 ReadVec4(this BinaryReader br) => new Vec4(br.ReadSingle(), br.ReadSingle(), br.ReadSingle(), br.ReadSingle());
		
		public static Reference<T> ReadRef<T>(this BinaryReader br, Wld wld) where T : class => new Reference<T>(wld, br.ReadInt32());
	}
}