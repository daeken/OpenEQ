using System.IO;
using System.Linq;
using OpenEQ.Common;

namespace OpenEQ.LegacyFileReader {
	public static class Extensions {
		public static string Stringify<T>(this T[] arr) => $"[{string.Join(", ", arr.Select(x => x.ToString()))}]";
		
		public static Reference<T> ReadRef<T>(this BinaryReader br, Wld wld) where T : class => new Reference<T>(wld, br.ReadInt32());
	}
}