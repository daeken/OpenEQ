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
using static System.Console;

namespace OpenEQ {
	class App {
		static void Main(string[] args) {
			var engine = new EngineCore();

			LoadZoneFile($"../ConverterApp/{args[0]}_oes.zip", engine);
			
			engine.Run();
		}

		static void LoadZoneFile(string path, EngineCore engine) {
			using(var zip = ZipFile.OpenRead(path)) {
				using(var ms = new MemoryStream()) {
					using(var temp = zip.GetEntry("main.oes").Open())
						temp.CopyTo(ms);
					ms.Position = 0;
					var zone = OESFile.Read<OESZone>(ms);
					WriteLine($"Loading {zone.Name}");
					
					engine.Add(FromMeshes(FromSkin(zone.Find<OESSkin>().First(), zip), new[] { Matrix4x4.Identity }, zone.Find<OESStaticMesh>()));

					var objInstances = zone.Find<OESObject>().ToDictionary(x => x, x => new List<Matrix4x4>());
					zone.Find<OESInstance>().ForEach(inst => {
						objInstances[inst.Object].Add(Matrix4x4.CreateScale(inst.Scale) * Matrix4x4.CreateFromQuaternion(inst.Rotation) * Matrix4x4.CreateTranslation(inst.Position));
					});
					foreach(var (obj, instances) in objInstances) {
						engine.Add(FromMeshes(
							FromSkin(obj.Find<OESSkin>().First(), zip), 
							instances.ToArray(), 
							obj.Find<OESStaticMesh>()
						));
					}
					
					zone.Find<OESLight>().ForEach(light => engine.AddLight(light.Position, light.Radius, light.Attenuation, light.Color));
				}
			}
		}

		static Model FromMeshes(IReadOnlyList<Material> mats, Matrix4x4[] instances, IEnumerable<OESStaticMesh> meshes) {
			var model = new Model();
			meshes.ForEach((sm, i) => model.Add(new Mesh(mats[i], sm.VertexBuffer.ToArray(), sm.IndexBuffer.ToArray(), instances)));
			return model;
		}

		static List<Material> FromSkin(OESSkin skin, ZipArchive zip) =>
			skin.Find<OESMaterial>().Select(mat => {
				var effect = mat.Find<OESEffect>().FirstOrDefault();
				effect = effect ?? new OESEffect("default");

				var textures = mat.Find<OESTexture>().Select(x => {
					using(var tzs = zip.GetEntry(x.Filename).Open())
						return Png.Decode(tzs);
				}).ToArray();
				
				switch(effect.Name) {
					case "default":
					case "animated":
						var aniSpeed = effect.Name == "animated" ? (uint) effect["speed"] / 1000f : 0;
						if(mat.Transparent)
							return mat.AlphaMask
								? (Material) new ForwardDiffuseMaskedMaterial(textures, aniSpeed)
								: new ForwardDiffuseMaterial(textures, aniSpeed);
						else
							return mat.AlphaMask
								? (Material) new DeferredDiffuseMaskedMaterial(textures, aniSpeed)
								: new DeferredDiffuseMaterial(textures, aniSpeed);
					case "diffuse+normal":
						return new DeferredDiffuseNormalMaterial(textures);
					case "fire":
						return new FireMaterial();
					default:
						throw new NotImplementedException($"Unknown OESEffect name: {effect.Name}");
				}
			}).ToList();
	}
}