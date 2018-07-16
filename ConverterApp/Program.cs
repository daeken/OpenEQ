using System;
using System.Diagnostics;
using System.IO;
using OpenEQ.ConverterCore;

namespace OpenEQ.ConverterApp {
	class Program {
		static void Main(string[] args) {
			var sw = new Stopwatch();
			Console.WriteLine("Starting conversion");
			sw.Start();
			var converter = new Converter(args[0]);
			var type = converter.Convert(args[1]);
			sw.Stop();
			switch(type) {
				case ConvertedType.None: Console.WriteLine("Conversion failed"); break;
				case ConvertedType.Zone: Console.WriteLine("Zone converted"); break;
				case ConvertedType.Characters: Console.WriteLine("Characters converted"); break;
			}
			Console.WriteLine($"Took {sw.ElapsedMilliseconds} ms");
		}
	}
}