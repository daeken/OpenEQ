using System;
using System.Collections.Generic;
using MoreLinq;

namespace OpenEQ.Engine {
	public class Model {
		public readonly List<Mesh> Meshes = new List<Mesh>();

		Mat4 Transformation = Mat4.Identity;

		public Model() {}
		
		Model(List<Mesh> meshes, Mat4 transformation) {
			Meshes = meshes;
			Transformation = transformation;
		}

		public void SetProperties(Vec3 translate, Vec3 scale, Vec3 rotate) {
			Transformation = Mat4.Scale(scale) * Mat4.RotationX(rotate.X) * Mat4.RotationY(rotate.Y) * Mat4.RotationZ(rotate.Z) * Mat4.Translation(translate);
		}

		public void Add(Mesh mesh) => Meshes.Add(mesh);

		public void Draw() => Meshes.ForEach(mesh => mesh.Draw(Transformation));
		
		public Model Clone() => new Model(Meshes, Transformation);
	}
}