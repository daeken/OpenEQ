using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using OpenEQ.Common;
using OpenEQ.LegacyFileReader;
using static System.Console;

namespace OpenEQ.ConverterCore {
	public class Converter {
		public string BasePath;
		
		public Converter(string basePath) => BasePath = basePath;

		public bool ConvertZone(string name) {
			var fns = FindFiles($"{name}*.s3d").Where(fn => !fn.Contains("_chr")).ToList();
			if(!fns.Contains($"{name}_obj.s3d")) return false;

			var s3ds = fns.AsParallel().Select(fn => new S3D(fn, File.OpenRead(Filename(fn)))).ToList();
			var wlds = s3ds.AsParallel().Select(s3d => s3d.Where(fn => fn.EndsWith(".wld")).Select(fn => new Wld(s3d, fn)))
				.SelectMany(x => x).ToList();

			var zone = new OESZone(name);
			var skin = new OESSkin();
			zone.Add(skin);
			
			foreach(var wld in wlds) {
				if(wld.Filename != name + ".wld") continue;
				var zonemesh = new Mesh();
				foreach(var (meshname, meshfrag) in wld.GetFragments<Fragment36>())
					zonemesh.Add(new MeshPiece(meshfrag));
				foreach(var (vb, ib, collidable, texture) in zonemesh.Bake()) {
					zone.Add(new OESStaticMesh(collidable, ib, vb));
					skin.Add(new OESMaterial(false, false, false) {
						new OESTexture(texture.Filenames.First())
					});
				}
			}
			
			OESFile.Write(File.OpenWrite($"{name}.oes"), zone);
			
			return true;
		}

		List<string> FindFiles(string pattern) => Directory.GetFiles(BasePath, pattern).Select(Path.GetFileName).ToList();

		string Filename(string name) => Path.Join(BasePath, name);
		bool Exists(string name) => File.Exists(Filename(name));
	}
}