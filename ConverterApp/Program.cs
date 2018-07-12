using System;
using System.IO;
using OpenEQ.ConverterCore;

namespace OpenEQ.ConverterApp {
	class Program {
		static void Main(string[] args) {
			var converter = new Converter(args[0]);
			Console.WriteLine(converter.ConvertZone(args[1]) ? "Zone converted" : "Zone not found");
		}
	}
}