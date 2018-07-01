using System;
using System.Collections.Generic;
using System.Linq;
using OpenEQ.Engine;
using static System.Console;

namespace OpenEQ {
	class Program {
		static void Main(string[] args) {
			var engine = new EngineCore();
			var zone = new ZoneReader($"../converter/{args[0]}.zip");
			var materials = zone.Materials.Select(mat => new Material((MaterialFlag) mat.Flags, mat.Param, mat.Textures.ToArray())).ToArray();
			var models = zone.Objects.Select(zobj => BuildModel(zobj, materials)).ToList();
			engine.Add(models[0]);
			zone.Placeables.ForEach(p => {
				var model = models[p.ObjId].Clone();
				model.SetProperties(p.Position, p.Scale, p.Rotation);
				engine.Add(model);
			});
			/*var smesh = Mesh.Sphere(null);
			zone.Lights.ForEach(p => {
				WriteLine($"Light {p.Flags:X}");
				var model = new Model();
				model.Add(smesh);
				model.SetProperties(p.Position, new Vec3(p.Radius / p.Attenuation * 10), Vec3.Zero);
				engine.Add(model);
			});*/
			engine.Run();
		}
		
		static Model BuildModel(List<ZoneMesh> zmeshes, Material[] materials) {
			var model = new Model();
			foreach(var zmesh in zmeshes)
				model.Add(new Mesh(materials[zmesh.MatId], zmesh.VertexBuffer, zmesh.Indices));
			return model;
		}
	}
}