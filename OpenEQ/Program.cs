using System;
using System.Collections.Generic;
using System.Linq;
using OpenEQ.Engine;
using MoreLinq;
using static System.Console;

namespace OpenEQ {
	class Program {
		static void Main(string[] args) {
			var engine = new EngineCore();
			var zone = new ZoneReader($"../converter/{args[0]}.zip");
			var materials = zone.Materials.Select(mat => new Material((MaterialFlag) mat.Flags, mat.Param, mat.Textures.ToArray())).ToArray();
			
			var objInstances = zone.Objects.Select(_ => new List<Mat4>()).ToArray();
			objInstances.First().Add(Mat4.Identity);

			zone.Placeables.ForEach(p =>
				objInstances[p.ObjId].Add(Mat4.Scale(p.Scale) * Mat4.RotationX(p.Rotation.X) * Mat4.RotationY(p.Rotation.Y) * Mat4.RotationZ(p.Rotation.Z) * Mat4.Translation(p.Position)));

			objInstances.ForEach((list, i) => {
				engine.Add(BuildModel(zone.Objects[i], materials, list.ToArray()));
			});
			
			WriteLine($"Loading {zone.Lights.Count} lights");
			zone.Lights.ForEach(lp => {
				engine.AddLight(lp.Position, lp.Radius, lp.Attenuation, lp.Color);
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
		
		static Model BuildModel(List<ZoneMesh> zmeshes, Material[] materials, Mat4[] instances) {
			var model = new Model();
			foreach(var zmesh in zmeshes)
				model.Add(new Mesh(materials[zmesh.MatId], zmesh.VertexBuffer, zmesh.Indices, instances));
			return model;
		}
	}
}