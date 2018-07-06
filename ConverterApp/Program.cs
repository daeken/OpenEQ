using System;
using System.IO;
using LegacyFileReader;

namespace ConverterApp {
	class Program {
		static void Main(string[] args) {
			using(var fp = File.OpenRead(args[0])) {
				var s3d = new S3D(fp);
				var wld = new Wld(s3d.Open("gfaydark_obj.wld"));
			}
		}
	}
}