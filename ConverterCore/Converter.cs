using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using ImageLib;
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

			var zn = $"{name}_oes.zip";
			if(File.Exists(zn)) File.Delete(zn);
			using(var zip = ZipFile.Open(zn, ZipArchiveMode.Create)) {
				var zone = new OESZone(name);
				var skin = new OESSkin();
				zone.Add(skin);

				foreach(var wld in wlds) {
					if(wld.Filename != name + ".wld") continue;
					var zonemesh = new Mesh();
					foreach(var (meshname, meshfrag) in wld.GetFragments<Fragment36>())
						zonemesh.Add(new MeshPiece(meshfrag));
					var baked = zonemesh.Bake();
					var textureFns = baked.Select(x => x.Texture.Filenames).SelectMany(x => x).OrderBy(x => x)
						.Distinct();
					var textureMap = textureFns.AsParallel().Select(fn => (fn, ConvertTexture(wld.S3D, zip, fn)))
						.ToDictionary();
					foreach(var (vb, ib, collidable, texture) in baked) {
						zone.Add(new OESStaticMesh(collidable, ib, vb));
						skin.Add(new OESMaterial(false, false, false) {
							new OESTexture(texture.Filenames.First())
						});
					}
				}

				OESFile.Write(zip.CreateEntry("main.oes", CompressionLevel.Optimal).Open(), zone);
			}

			var idat = new byte[800 * 600 * 3];
			var i = 0;
			for(var y = 0; y < 600; ++y)
				for(var x = 0; x < 800; ++x) {
					var fx = x / 799.0;
					var fy = y / 599.0;
					idat[i++] = (byte) Math.Round(fx * 255);
					idat[i++] = (byte) Math.Round(fy * 255);
					idat[i++] = (byte) Math.Round(fx * fy * 255);
				}
			
			var img = new Image(ColorMode.Rgb, (800, 600), idat);
			using(var fp = File.OpenWrite("test.png"))
				Png.Encode(img, fp);
			
			return true;
		}

		string ConvertTexture(S3D s3d, ZipArchive zip, string fn) {
			byte[] data;
			lock(s3d) data = s3d[fn];
			
			return fn;
		}

		List<string> FindFiles(string pattern) => Directory.GetFiles(BasePath, pattern).Select(Path.GetFileName).ToList();

		string Filename(string name) => Path.Join(BasePath, name);
		bool Exists(string name) => File.Exists(Filename(name));
	}
}