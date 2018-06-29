using System;
using System.Collections.Generic;
using System.Linq;
using OpenEQ.Engine;

namespace OpenEQ {
	class Program {
		static void Main(string[] args) {
			var engine = new EngineCore();
			var zone = new ZoneReader("../converter/gfaydark.zip");
			var materials = zone.Materials.Select(mat => new Material((MaterialFlag) mat.Flags, mat.Param, mat.Textures.ToArray())).ToArray();
			engine.Add(BuildModel(zone.Objects[0], materials));
			engine.Run();
		}

		static Model BuildModel(List<ZoneMesh> zmeshes, Material[] materials) {
			var model = new Model();
			foreach(var zmesh in zmeshes)
				model.Add(new Mesh(materials[zmesh.MatId], zmesh.Positions, zmesh.Normals, zmesh.TexCoords, zmesh.Indices));
			return model;
		}
	}
}