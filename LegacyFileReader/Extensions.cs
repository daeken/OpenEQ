using System.Linq;

namespace LegacyFileReader {
	public static class Extensions {
		public static string Stringify<T>(this T[] arr) => $"[{string.Join(", ", arr.Select(x => x.ToString()))}]";
	}
}