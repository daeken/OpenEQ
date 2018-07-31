using static System.Console;

namespace OpenEQ.LegacyFileReader {
	public static class Debugging {
		static string Escape(string v) => v.Replace("&", "&amp;").Replace("\"", "&quot;").Replace("<", "&lt;").Replace(">", "&gt;");
		public static void OutputHTML(Wld wld) {
			foreach(var (name, frag) in wld.Fragments) {
				WriteLine($"<li>{(string.IsNullOrEmpty(name) ? "" : $"<i>{Escape(name)}</i> - ")}{Escape(frag?.ToString() ?? "NULL")}</li>");
			}
		}
	}
}