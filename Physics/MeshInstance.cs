using System.Linq;
using System.Numerics;

namespace Physics {
	public struct MeshInstance {
		public readonly Mesh Mesh;
		public readonly Matrix4x4 Transform;

		public MeshInstance(Mesh mesh, Matrix4x4 transform) {
			Mesh = mesh;
			Transform = transform;
		}

		public Mesh Bake() {
			var transform = Transform;
			return new Mesh(Mesh.Triangles.Select(tri =>
				new Triangle(
					Vector3.Transform(tri.A, transform), 
					Vector3.Transform(tri.B, transform), 
					Vector3.Transform(tri.C, transform))));
		}
	}
}