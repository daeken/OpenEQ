using System.Numerics;

namespace Physics {
	public struct Triangle : ICollidable {
		public readonly Vector3 A, B, C;
		public readonly Vector3 Normal;

		public Vector3[] AsArray => new[] { A, B, C }; 

		public Triangle(Vector3 a, Vector3 b, Vector3 c) {
			A = a;
			B = b;
			C = c;

			Normal = Vector3.Cross(b - a, c - a);
		}
	}
}