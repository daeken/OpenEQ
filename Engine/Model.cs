using System;
using System.Collections.Generic;
using System.Numerics;
using MoreLinq;

namespace OpenEQ.Engine {
	public class Model {
		public readonly List<Mesh> Meshes = new List<Mesh>();
		public readonly bool IsFixed;
		public Matrix4x4 Transform = Matrix4x4.Identity;

		public Model(bool isFixed = true) => IsFixed = isFixed;

		public void Add(Mesh mesh) => Meshes.Add(mesh);

		public void Draw(Matrix4x4 projView, bool forward) => Meshes.ForEach(mesh => mesh.Draw(projView, Transform, forward));
	}
}