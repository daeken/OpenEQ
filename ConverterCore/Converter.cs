using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Numerics;
using System.Runtime.Serialization;
using System.Security.Cryptography;
using ImageLib;
using MoreLinq;
using OpenEQ.Common;
using OpenEQ.LegacyFileReader;
using static System.Console;
using Extensions = OpenEQ.Common.Extensions;

namespace OpenEQ.ConverterCore {
	public enum ConvertedType {
		None, 
		Zone, 
		Characters
	}
	
	public class Converter {
		public string BasePath;

		Dictionary<(string, string), string> TextureMap;

		public Converter(string basePath) => BasePath = basePath;

		public ConvertedType Convert(string name) {
			if(name.EndsWith("_chr"))
				return ConvertCharacters(name) ? ConvertedType.Characters : ConvertedType.None;
			if(ConvertWldZone(name) || ConvertEqgZone(name))
				return ConvertedType.Zone;
			return ConvertedType.None;
		}

		bool ConvertWldZone(string name) {
			var fns = FindFiles($"{name}.s3d").Concat(FindFiles($"{name}_*.s3d")).Where(fn => !fn.Contains("_chr")).ToList();
			if(!fns.Contains($"{name}_obj.s3d")) return false;

			var s3ds = fns.AsParallel().Select(fn => new S3D(fn, File.OpenRead(Filename(fn)))).ToList();
			var wlds = s3ds.AsParallel().Select(s3d => s3d.Where(fn => fn.EndsWith(".wld")).Select(fn => new Wld(s3d, fn)))
				.SelectMany(x => x).ToList();
			
			var zn = $"{name}_oes.zip";
			if(File.Exists(zn)) File.Delete(zn);
			using(var zip = ZipFile.Open(zn, ZipArchiveMode.Create)) {
				var texs = wlds
					.Select(x =>
						x.GetFragments<Fragment03>().Select(y => y.Fragment.Filenames.Select(z => (x.S3D, z))))
					.SelectMany(x => x).SelectMany(x => x).Distinct();
				TextureMap = texs.AsParallel().Select(x => ((x.Item1.Filename, x.Item2), ConvertTexture(x.Item1, zip, x.Item2))).ToDictionary();

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
						var objname = instance.Reference.Value?.Replace("_ACTORDEF", "");
						if(objname == null || !objMap.ContainsKey(objname)) continue;
						zone.Add(new OESInstance(
							objMap[objname], instance.Position, instance.Scale,
							Quaternion.CreateFromAxisAngle(new Vector3(0, 0, 1), instance.Rotation.Z) * 
							Quaternion.CreateFromAxisAngle(new Vector3(0, 1, 0), instance.Rotation.Y) * 
							Quaternion.CreateFromAxisAngle(new Vector3(1, 0, 0), instance.Rotation.X)
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

		bool ConvertCharacters(string name) {
			var fns = FindFiles($"{name}*.s3d").ToList();
			if(fns.Count == 0) return false;

			var s3ds = fns.AsParallel().Select(fn => new S3D(fn, File.OpenRead(Filename(fn)))).ToList();
			var wlds = s3ds.AsParallel().Select(s3d => s3d.Where(fn => fn.EndsWith(".wld")).Select(fn => new Wld(s3d, fn)))
				.SelectMany(x => x).ToList();

			foreach(var wld in wlds) {
				WriteLine($"<h1>{wld.Filename}</h1>");
				WriteLine("<il>");
				Debugging.OutputHTML(wld);
				WriteLine("</il>");
			}

			var zn = $"{name}_oes.zip";
			if(File.Exists(zn)) File.Delete(zn);
			using(var zip = ZipFile.Open(zn, ZipArchiveMode.Create)) {
				var root = new OESRoot();
				OESFile.Write(zip.CreateEntry("main.oes", CompressionLevel.Optimal).Open(), root);
			}

			return true;
		}

		bool ConvertEqgZone(string name) {
			var ename = $"{name}.eqg";
			if(!Exists(ename)) return false;
			
			var eqg = new S3D(ename, File.OpenRead(Filename(ename)));
			Zon zon;
			var zname = $"{name}.zon";
			if(eqg.Contains(zname))
				zon = new Zon(eqg, eqg.Open(zname));
			else if(Exists(zname))
				zon = new Zon(eqg, File.OpenRead(zname));
			else
				return false;
			
			var zn = $"{name}_oes.zip";
			if(File.Exists(zn)) File.Delete(zn);
			using(var zip = ZipFile.Open(zn, ZipArchiveMode.Create)) {
				var texs = zon.Objects.Select(x => x.Materials.Values.Select(y => 
					y.Properties.Values.Where(z => z is string w && w.ToLower().EndsWith(".dds")).Select(z => 
						(string) z)).SelectMany(y => y)).SelectMany(x => x).OrderBy(x => x).Distinct();
				TextureMap = texs.AsParallel()
					.Select(x => ((ename, x), ConvertTexture(eqg, zip, x))).ToDictionary();

				var zone = new OESZone(name);
				var objs = zon.Objects.Select((obj, i) => {
					var root = obj.IsTer ? (OESChunk) zone : new OESObject();
					if(root != zone)
						zone.Add(root);
					var skin = new OESSkin();
					root.Add(skin);
					obj.Meshes.ForEach(mesh => {
						if(!obj.Materials.ContainsKey(mesh.Key.MatIndex)) return;
						var mat = obj.Materials[mesh.Key.MatIndex];
						if(mat.Properties.ContainsKey("e_TextureNormal0") && (string) mat.Properties["e_TextureNormal0"] != "None") {
							skin.Add(new OESMaterial(false, false, false) {
								new OESEffect("diffuse+normal"),
								new OESTexture(TextureMap[(ename, (string) mat.Properties["e_TextureDiffuse0"])]),
								new OESTexture(TextureMap[(ename, (string) mat.Properties["e_TextureNormal0"])])
							});
						} else
							skin.Add(new OESMaterial(false, false, false) { new OESTexture(TextureMap[(ename, (string) mat.Properties["e_TextureDiffuse0"])]) });
						var (vb, ib) = OptimizeBuffers(obj.VertexBuffer, mesh.Value);
						root.Add(new OESStaticMesh(mesh.Key.Collidable, ib, vb));
					});
					return obj.IsTer ? null : root;
				}).ToList();
				
				zon.Placeables.ForEach(instance => {
					if(objs.Count <= instance.ObjId || objs[instance.ObjId] == null) return;
					
					zone.Add(new OESInstance(
						(OESObject) objs[instance.ObjId], 
						instance.Position, 
						new Vector3(instance.Scale), 
						Quaternion.CreateFromAxisAngle(new Vector3(0, 0, 1), instance.Rotation.X) * 
						Quaternion.CreateFromAxisAngle(new Vector3(0, 1, 0), instance.Rotation.Y) * 
						Quaternion.CreateFromAxisAngle(new Vector3(1, 0, 0), instance.Rotation.Z)));
				});
				
				zon.Lights.ForEach(light => zone.Add(new OESLight(light.Position, light.Color, light.Radius, 200)));
				
				OESFile.Write(zip.CreateEntry("main.oes", CompressionLevel.Optimal).Open(), zone);
			}

			return true;
		}

		(float[] VertexBuffer, uint[] IndexBuffer) OptimizeBuffers(IReadOnlyList<float> vb, IReadOnlyList<uint> ib) {
			var pvb = (0, vb.Count, 8).Range().Select(i => (
				new Vector3(vb[i++], vb[i++], vb[i++]),
				new Vector3(vb[i++], vb[i++], vb[i++]), 
				new Vector2(vb[i++], vb[i])
			)).ToList();

			var ovb = new List<float>();
			var vertMap = new Dictionary<(Vector3, Vector3, Vector2), uint>();

			uint Add((Vector3, Vector3, Vector2) vert) {
				if(vertMap.ContainsKey(vert)) return vertMap[vert];

				var ind = vertMap[vert] = (uint) vertMap.Count;
				ovb.AddRange(vert.Item1.AsArray());
				ovb.AddRange(vert.Item2.AsArray());
				ovb.AddRange(vert.Item3.AsArray());
				return ind;
			}

			var oib = ib.Select(x => Add(pvb[(int) x])).ToArray();
			return (ovb.ToArray(), oib);
		}

		void CreateMeshAndSkin(Wld wld, ZipArchive zip, OESChunk target, IEnumerable<MeshPiece> pieces) {
			var mesh = new Mesh();
			pieces.ForEach(mesh.Add);
			var baked = mesh.Bake();
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
					texture.Filenames.ForEach(fn => mat.Add(new OESTexture(TextureMap[(wld.S3D.Filename, fn)])));
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
				var entry = zip.CreateEntry(ofn, CompressionLevel.Optimal).Open();
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