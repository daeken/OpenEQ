#pragma warning disable 414
namespace System.IO {
	public static class Path2 {
		internal const int MAX_PATH = 260;

		internal const int MAX_DIRECTORY_PATH = 248;

	    /// <summary>
	    ///     Provides a platform-specific character used to separate directory levels in a path string that reflects a
	    ///     hierarchical file system organization.
	    /// </summary>
	    /// <filterpriority>1</filterpriority>
	    public static readonly char DirectorySeparatorChar;

	    /// <summary>
	    ///     Provides a platform-specific alternate character used to separate directory levels in a path string that
	    ///     reflects a hierarchical file system organization.
	    /// </summary>
	    /// <filterpriority>1</filterpriority>
	    public static readonly char AltDirectorySeparatorChar;

	    /// <summary>Provides a platform-specific volume separator character.</summary>
	    /// <filterpriority>1</filterpriority>
	    public static readonly char VolumeSeparatorChar;

		internal static readonly char[] TrimEndChars;

		static readonly char[] RealInvalidPathChars;

		static readonly char[] InvalidFileNameChars;

	    /// <summary>A platform-specific separator character used to separate path strings in environment variables.</summary>
	    /// <filterpriority>1</filterpriority>
	    public static readonly char PathSeparator;

		internal static readonly int MaxPath;

		static readonly int MaxDirectoryLength;

		internal static readonly int MaxLongPath;

		static readonly string Prefix;

		static readonly char[] s_Base32Char;

		static Path2() {
			DirectorySeparatorChar = '\\';
			AltDirectorySeparatorChar = '/';
			VolumeSeparatorChar = ':';
			TrimEndChars = new[] { '\t', '\n', '\v', '\f', '\r', ' ', '\u0085', '\u00A0' };
			RealInvalidPathChars = new[] {
				'\"', '<', '>', '|', '\0', '\u0001', '\u0002', '\u0003', '\u0004', '\u0005', '\u0006', '\a', '\b', '\t',
				'\n', '\v', '\f', '\r', '\u000E', '\u000F', '\u0010', '\u0011', '\u0012', '\u0013', '\u0014', '\u0015',
				'\u0016', '\u0017', '\u0018', '\u0019', '\u001A', '\u001B', '\u001C', '\u001D', '\u001E', '\u001F'
			};
			InvalidFileNameChars = new[] {
				'\"', '<', '>', '|', '\0', '\u0001', '\u0002', '\u0003', '\u0004', '\u0005', '\u0006', '\a', '\b', '\t',
				'\n', '\v', '\f', '\r', '\u000E', '\u000F', '\u0010', '\u0011', '\u0012', '\u0013', '\u0014', '\u0015',
				'\u0016', '\u0017', '\u0018', '\u0019', '\u001A', '\u001B', '\u001C', '\u001D', '\u001E', '\u001F', ':',
				'*', '?', '\\', '/'
			};
			PathSeparator = ';';
			MaxPath = 260;
			MaxDirectoryLength = 255;
			MaxLongPath = 32000;
			Prefix = "\\\\?\\";
			s_Base32Char = new[] {
				'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u',
				'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5'
			};
		}

		internal static void CheckInvalidPathChars(string path, bool checkAdditional = false) {
			if(path != null) {
				if(!HasIllegalCharacters(path, checkAdditional)) {
				} else
					throw new ArgumentException("The path has invalid characters.", "path");
			} else
				throw new ArgumentNullException("path");
		}

		internal static bool HasIllegalCharacters(string path, bool checkAdditional) {
			var num = 0;
			while(num < path.Length) {
				int num1 = path[num];
				if(num1 == 34 || num1 == 60 || num1 == 62 || num1 == 124 || num1 < 32) return true;

				if(!checkAdditional || num1 != 63 && num1 != 42)
					num++;
				else
					return true;
			}

			return false;
		}

		public static string GetFileName(string path) {
			char chr;
			if(path != null) {
				CheckInvalidPathChars(path, false);
				var length = path.Length;
				var num = length;
				do {
					var num1 = num - 1;
					num = num1;
					if(num1 < 0) return path;
					chr = path[num];
				} while(chr != DirectorySeparatorChar && chr != AltDirectorySeparatorChar &&
				        chr != VolumeSeparatorChar);

				return path.Substring(num + 1, length - num - 1);
			}

			return path;
		}
	}
}