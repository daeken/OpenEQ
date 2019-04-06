using System.IO;
using System.IO.Compression;
using MoreLinq;
using OpenEQ.Common;
using static System.Console;

namespace OesDumper {
	internal class Program {
		static void Main(string[] args) {
			using(var zip = ZipFile.OpenRead(args[0])) {
				using(var ms = new MemoryStream()) {
					using(var temp = zip.GetEntry("main.oes")?.Open())
						temp?.CopyTo(ms);
					ms.Position = 0;
					
					void Display(OESChunk chunk, int indentation) {
						WriteLine($"{new string('\t', indentation)}{chunk}");
						chunk.ForEach(x => Display(x, indentation + 1));
					}
					
					Display(OESFile.Read(ms), 0);
				}
			}
		}
	}
}