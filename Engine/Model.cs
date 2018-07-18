using System;
using System.Collections.Generic;
using System.Numerics;
using MoreLinq;

namespace OpenEQ.Engine {
	public class Model {
		readonly List<Mesh> Meshes = new List<Mesh>();

		public void Add(Mesh mesh) => Meshes.Add(mesh);

		public void Draw(Matrix4x4 projView, bool forward) => Meshes.ForEach(mesh => mesh.Draw(projView, forward));
	}
}