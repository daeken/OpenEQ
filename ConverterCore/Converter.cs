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

			/*foreach(var wld in wlds) {
				WriteLine($"<h1>{wld.Filename}</h1>");
				WriteLine("<il>");
				Debugging.OutputHTML(wld);
				WriteLine("</il>");
			}*/

			var zn = $"{name}_oes.zip";
			if(File.Exists(zn)) File.Delete(zn);
			using(var zip = ZipFile.Open(zn, ZipArchiveMode.Create)) {
				var root = new OESRoot();
				foreach(var wld in wlds)
					foreach(var (aname, actor) in wld.GetFragments<Fragment14>()) {
						WriteLine(aname);
						var model = new OESCharacter(aname.Substring(0, aname.Length - "_ACTORDEF".Length));
						root.Add(model);
						var skin = new OESSkin();
						model.Add(skin);
						foreach(var elem in actor.References)
							switch(elem.Value) {
								case Fragment11 f11:
									GenerateAnimatedMeshes(wld, zip, model, skin, f11.Reference.Value);
									break;
								default:
									WriteLine($"Unknown reference from 0x14 fragment to {elem.Value}");
									break;
							}
					}
				OESFile.Write(zip.CreateEntry("main.oes", CompressionLevel.Optimal).Open(), root);
			}

			return true;
		}

		class AniTreePrecursor {
			public uint Index;
			public (Vector3? Rotate, Vector3? Translate)[] Frames;
			public AniTreePrecursor[] Children;
		}

		class AniTreeFrame {
			public uint Index;
			public (Vector3? Rotate, Vector3? Translate) Transform;
			public AniTreeFrame[] Children;
		}

		void GenerateAnimatedMeshes(Wld wld, ZipArchive zip, OESCharacter model, OESSkin skin, Fragment10 f10) {
			var prefixes = new List<string> { "" };
			var rootName = f10.Tracks[0].PieceTrack.Name;
			
			foreach(var f13 in wld.GetFragments<Fragment13>())
				if(f13.Name != rootName && f13.Name.EndsWith(rootName))
					prefixes.Add(f13.Name.Substring(0, f13.Name.Length - rootName.Length));
			prefixes = prefixes.Distinct().ToList();

			AniTreePrecursor BuildAniTreePrecursor(string prefix, uint index) {
				var track = f10.Tracks[index];
				var ptref = track.PieceTrack;
				var piecetrack = ptref.Value;
				if(prefix != "" && wld.GetFragment<Fragment13>(prefix + ptref.Name) is Fragment13 rep)
					piecetrack = rep;
				return new AniTreePrecursor { Index = index, Frames = piecetrack.Reference.Value.Frames, Children = track.Children.Select(i => BuildAniTreePrecursor(prefix, (uint) i)).ToArray() };
			}

			AniTreeFrame[] BuildFrameTree(AniTreePrecursor pc) {
				int GetMaxFrames(AniTreePrecursor pct) =>
					pct.Children.Select(GetMaxFrames).Concat(new[] { pct.Frames.Length }).Max();

				AniTreeFrame BuildFrame(AniTreePrecursor pct, int frame) =>
					new AniTreeFrame { Index = pct.Index, Transform = pct.Frames[pct.Frames.Length > 1 ? frame : 0], Children = pct.Children.Select(x => BuildFrame(x, frame)).ToArray() };

				return GetMaxFrames(pc).Times(i => BuildFrame(pc, i)).ToArray();
			}
			
			var trees = prefixes.Select(x => (x, BuildFrameTree(BuildAniTreePrecursor(x, 0)))).ToDictionary();

			var animationBuffers = new Dictionary<string, List<List<List<float>>>>();
			foreach(var (name, frames) in trees) {
				var frameBuffers = animationBuffers[name] = f10.Meshes.Length.Times(() => new List<List<float>>()).ToList();
				foreach(var frame in frames) {
					var matrices = new Dictionary<uint, Matrix4x4>();

					void BuildBoneMatrices(AniTreeFrame cur, Matrix4x4 mat) {
						if(cur.Transform.Translate != null)
							mat = Matrix4x4.CreateTranslation(cur.Transform.Translate.Value) * mat;
						matrices[cur.Index] = mat;
						cur.Children.ForEach(x => BuildBoneMatrices(x, mat));
					}
					BuildBoneMatrices(frame, Matrix4x4.Identity);

					f10.Meshes.ForEach((mr, i) => {
						var curBuffer = new List<float>();
						var mesh = mr.Value.Reference.Value;
						var offset = 0U;
						foreach(var (count, index) in mesh.VertBones) {
							var mat = matrices[index];
							for(var j = 0; j < count; ++j) {
								curBuffer.AddRange(Vector3.Transform(mesh.Vertices[offset + j], mat).AsArray());
								curBuffer.AddRange(Vector3.Transform(mesh.Normals[offset + j], mat).AsArray());
								curBuffer.AddRange(mesh.TexCoords[offset + j].AsArray());
							}
							offset += count;
						}
						frameBuffers[i].Add(curBuffer);
					});
				}
			}

			(List<uint>, Dictionary<string, OESAnimationBuffer>) RewriteBuffers(List<uint> indices, Dictionary<string, List<List<float>>> vertices) {
				var indmap = new Dictionary<uint, uint>();
				var oind = indices.Select(x => indmap.ContainsKey(x) ? indmap[x] : (indmap[x] = (uint) indmap.Count)).ToList();
				indmap = indmap.Select(kv => (kv.Value, kv.Key)).ToDictionary();

				List<float> Remap(List<float> vb) =>
					indmap.Keys.OrderBy(x => x).Select(x => vb.Skip((int) x * 8).Take(8)).SelectMany(x => x).ToList();
				
				var overts = vertices.Select(kv => (kv.Key, new OESAnimationBuffer(kv.Value.Select(Remap).ToList()))).ToDictionary();
				return (oind, overts);
			}

			var asets = prefixes.Where(x => x != "").Select(x => (x, new OESAnimationSet(x, 0f))).ToDictionary();
			f10.Meshes.Length.Times(i => {
				var meshf = f10.Meshes[i].Value.Reference.Value;
				var omats = meshf.TextureListReference.Value.References.Select(matref => {
					var tfn = matref.Value.Reference.Value.Reference.Value.References[0].Value.Filenames[0];
					return new OESMaterial(false, false, false) { new OESTexture(ConvertTexture(wld.S3D, zip, tfn)) };
				}).ToList();
				var offset = 0U;
				meshf.PolyTexs.ForEach(v => {
					var polys = meshf.Polygons.Skip((int) offset).Take((int) v.Count).Select(x => new[] { x.A, x.B, x.C }).SelectMany(x => x).ToList();
					offset += v.Count;
					skin.Add(omats[(int) v.Index]);
					var (ibuffer, vbuffers) = RewriteBuffers(
						polys, 
						animationBuffers.Select(kv => (kv.Key, kv.Value[i])).ToDictionary()
					);
					var amesh = new OESAnimatedMesh(true, ibuffer, (uint) vbuffers[""].VertexBuffers[0].Count);
					model.Add(amesh);
					foreach(var prefix in prefixes) {
						if(prefix == "")
							amesh.Add(vbuffers[""]);
						else
							asets[prefix].Add(vbuffers[prefix]);
					}
				});
			});
			asets.ForEach(kv => model.Add(kv.Value));
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
			var image = data[0] == 'B' && data[1] == 'M'
				? new Image(ColorMode.Rgb, (1, 1), new byte[] { 0xFF, 0xFF, 0 })
				: Dds.Load(data).Images[0];
			lock(zip) {
				var entry = zip.CreateEntry(ofn, CompressionLevel.Optimal).Open();
				Png.Encode(image, entry);
				entry.Close();
			}

			return ofn;
		}

		List<string> FindFiles(string pattern) => Directory.GetFiles(BasePath, pattern).Select(Path.GetFileName).ToList();

		string Filename(string name) => Path.Join(BasePath, name);
		bool Exists(string name) => File.Exists(Filename(name));
	}
}