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
			var success = converter.ConvertZone(args[1]);
			sw.Stop();
			Console.WriteLine(success ? "Zone converted" : "Zone not found");
			Console.WriteLine($"Took {sw.ElapsedMilliseconds} ms");
		}
	}
}