using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Numerics;
using ImageLib;
using OpenEQ.Engine;
using MoreLinq;
using OpenEQ.Common;
using OpenEQ.Materials;
using OpenEQ.Views;
using static System.Console;

namespace OpenEQ {
	class App {
		static void Main(string[] args) {
			var controller = new Controller();
			controller.LoadZone(args[0]);
			controller.LoadCharacter("gfaydark_chr", "WAS");
			controller.AddView(new StatusView(controller));
			//controller.AddView(new ModelMeshView(controller));
			controller.Start();
		}
	}
}