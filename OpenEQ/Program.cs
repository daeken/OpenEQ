using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using ImageLib;
using OpenEQ.Engine;
using MoreLinq;
using OpenEQ.Common;
using static System.Console;

namespace OpenEQ {
	class Program {
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
					
					engine.Add(FromMeshes(FromSkin(zone.Find<OESSkin>().First(), zip), new[] { Mat4.Identity }, zone.Find<OESStaticMesh>()));

					var objInstances = zone.Find<OESObject>().ToDictionary(x => x, x => new List<Mat4>());
					zone.Find<OESInstance>().ForEach(inst => {
						objInstances[inst.Object].Add(Mat4.Scale(inst.Scale) * inst.Rotation.ToMatrix() * Mat4.Translation(inst.Position));
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

		static Model FromMeshes(IReadOnlyList<Material> mats, Mat4[] instances, IEnumerable<OESStaticMesh> meshes) {
			var model = new Model();
			meshes.ForEach((sm, i) => model.Add(new Mesh(mats[i], sm.VertexBuffer.ToArray(), sm.IndexBuffer.ToArray(), instances)));
			return model;
		}

		static List<Material> FromSkin(OESSkin skin, ZipArchive zip) =>
			skin.Find<OESMaterial>().Select(mat =>
				new Material(
					(mat.Transparent ? MaterialFlag.Translucent : MaterialFlag.Normal) |
					(mat.AlphaMask ? MaterialFlag.Masked : MaterialFlag.Normal),
					0,
					mat.Find<OESTexture>().Select(x => {
						using(var tzs = zip.GetEntry(x.Filename).Open())
							return Png.Decode(tzs);
					}).ToArray()
				)).ToList();
	}
}