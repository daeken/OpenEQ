using System;
using System.Collections.Generic;
using MoreLinq;

namespace OpenEQ.Engine {
	public class Model {
		readonly List<Mesh> Meshes = new List<Mesh>();

		public void Add(Mesh mesh) => Meshes.Add(mesh);

		public void Draw() => Meshes.ForEach(mesh => mesh.Draw(Mat4.Identity));
	}
}