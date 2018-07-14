using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Security.Cryptography;
using ImageLib;
using MoreLinq;
using OpenEQ.Common;
using OpenEQ.LegacyFileReader;
using static System.Console;
using Extensions = OpenEQ.Common.Extensions;

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
				
				foreach(var wld in wlds) {
					if(wld.Filename != name + ".wld") continue;
					CreateMeshAndSkin(
						wld, zip, zone, 
						wld.GetFragments<Fragment36>().Select(mesh => new MeshPiece(mesh.Fragment))
					);
					break;
				}

				var objMap = new Dictionary<string, OESObject>();
				foreach(var wld in wlds) {
					if(wld.Filename == name + ".wld") continue;

					foreach(var (_objname, objmesh) in wld.GetFragments<Fragment36>()) {
						var objname = _objname.Replace("_DMSPRITEDEF", "");
						CreateMeshAndSkin(wld, zip, objMap[objname] = new OESObject(objname), new[] { new MeshPiece(objmesh) });
						zone.Add(objMap[objname]);
					}
				}

				foreach(var wld in wlds) {
					if(wld.Filename == name + ".wld") continue;

					foreach(var (instname, instance) in wld.GetFragments<Fragment15>()) {
						var objname = instance.Reference.Value.Replace("_ACTORDEF", "");
						if(!objMap.ContainsKey(objname)) continue;
						zone.Add(new OESInstance(
							objMap[objname], instance.Position, instance.Scale, 
							Quaternion.FromAxisAngle(new Vec3(0, 0, 1), -instance.Rotation.Z) * 
							Quaternion.FromAxisAngle(new Vec3(0, 1, 0), -instance.Rotation.Y) * 
							Quaternion.FromAxisAngle(new Vec3(1, 0, 0), -instance.Rotation.X)
						));
					}
				}

				foreach(var lf in wlds.First(x => x.Filename == "lights.wld").GetFragments<Fragment28>()) {
					var light = lf.Fragment;
					var sl = light.Reference.Value.Reference.Value;
					zone.Add(new OESLight(light.Pos, sl.Color, light.Radius, sl.Attenuation ?? 200));
				}

				OESFile.Write(zip.CreateEntry("main.oes", CompressionLevel.Optimal).Open(), zone);
			}

			return true;
		}

		void CreateMeshAndSkin(Wld wld, ZipArchive zip, OESChunk target, IEnumerable<MeshPiece> pieces) {
			var mesh = new Mesh();
			pieces.ForEach(mesh.Add);
			var baked = mesh.Bake();
			var textureFns = baked.Select(x => x.Texture.Filenames).SelectMany(x => x).OrderBy(x => x)
				.Distinct();
			var textureMap = Extensions.ToDictionary(textureFns.AsParallel().Select(fn => (fn, ConvertTexture(wld.S3D, zip, fn))));
			var skin = new OESSkin();
			target.Add(skin);
			foreach(var (vb, ib, collidable, texture) in baked) {
				if(texture.Flags == 0) continue; // TODO: Bake this in, but non-renderable. Collision mesh type?
				target.Add(new OESStaticMesh(collidable, ib, vb));
				var tf = texture.Flags;
				var masked = (tf & (2 | 8 | 16)) != 0;
				var transparent = (tf & (4 | 8)) != 0;
				if((tf & 0xFFFF) == 0x14) // TODO: Remove hack. Fixes tiger head in Halas
					masked = transparent = false;
				var isFire = texture.Filenames[0].ToLower() == "fire1.bmp";
				var mat = new OESMaterial(masked, transparent, isFire);
				if(isFire)
					mat.Add(new OESEffect("fire"));
				else {
					if(texture.Filenames.Count > 1)
						mat.Add(new OESEffect("animated") { ["speed"] = texture.AnimSpeed });
					texture.Filenames.ForEach(fn => mat.Add(new OESTexture(textureMap[fn])));
				}

				skin.Add(mat);
			}
		}

		string ConvertTexture(S3D s3d, ZipArchive zip, string fn) {
			fn = fn.Substring(0, fn.IndexOf('.') + 4);
			byte[] data;
			lock(s3d) data = s3d[fn];

			var md5 = string.Join("", MD5.Create().ComputeHash(data).Select(x => $"{x:X02}")).Substring(0, 10);

			var ofn = $"{fn.Split('.', 2)[0]}-{md5}.png";
			var dimg = Dds.Load(data);
			var scaled = dimg.Images[0];//.UpscaleFfmpeg(4);
			lock(zip) {
				var entry = zip.CreateEntry(ofn, CompressionLevel.NoCompression).Open();
				Png.Encode(scaled, entry);
				entry.Close();
			}

			return ofn;
		}

		List<string> FindFiles(string pattern) => Directory.GetFiles(BasePath, pattern).Select(Path.GetFileName).ToList();

		string Filename(string name) => Path.Join(BasePath, name);
		bool Exists(string name) => File.Exists(Filename(name));
	}
}