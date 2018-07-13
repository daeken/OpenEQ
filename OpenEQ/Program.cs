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
			
			/*var zone = new ZoneReader($"../converter/{args[0]}.zip");
			var materials = zone.Materials.Select(mat => new Material((MaterialFlag) mat.Flags, mat.Param, mat.Textures.ToArray())).ToArray();
			
			var objInstances = zone.Objects.Select(_ => new List<Mat4>()).ToArray();
			objInstances.First().Add(Mat4.Identity);

			zone.Placeables.ForEach(p => objInstances[p.ObjId].Add(
				Mat4.Scale(p.Scale) * Mat4.RotationZ(p.Rotation.Z) * Mat4.RotationY(p.Rotation.Y) * Mat4.RotationX(p.Rotation.X) * Mat4.Translation(p.Position)));

			objInstances.ForEach((list, i) => engine.Add(BuildModel(zone.Objects[i], materials, list.ToArray())));
			
			zone.Lights.ForEach(lp => engine.AddLight(lp.Position, lp.Radius, lp.Attenuation, lp.Color));*/
			
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
					var mats = zone.Find<OESSkin>().First().Find<OESMaterial>().Select(mat => 
						new Material(
							(mat.Transparent ? MaterialFlag.Translucent : MaterialFlag.Normal) | (mat.AlphaMask ? MaterialFlag.Masked : MaterialFlag.Normal), 
							0, 
							mat.Find<OESTexture>().Select(x => {
								using(var tzs = zip.GetEntry(x.Filename).Open())
									return Png.Decode(tzs);
							}).ToArray()
						)).ToList();

					var instances = new[] { Mat4.Identity };
					var model = new Model();
					zone.Find<OESStaticMesh>().ForEach((sm, i) => {
						model.Add(new Mesh(mats[i], sm.VertexBuffer.ToArray(), sm.IndexBuffer.ToArray(), instances));
					});
					engine.Add(model);
					
					zone.Find<OESLight>().ForEach(light => engine.AddLight(light.Position, light.Radius, light.Attenuation, light.Color));
				}
			}
		}
		
		static Model BuildModel(List<ZoneMesh> zmeshes, Material[] materials, Mat4[] instances) {
			var model = new Model();
			foreach(var zmesh in zmeshes)
				model.Add(new Mesh(materials[zmesh.MatId], zmesh.VertexBuffer, zmesh.Indices, instances));
			return model;
		}
	}
}