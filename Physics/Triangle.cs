using System.Collections.Generic;
using System.Numerics;

namespace Physics {
	public struct Triangle {
		public readonly Vector3 A, B, C;

		public Vector3[] AsArray => new[] { A, B, C }; 

		public Triangle(Vector3 a, Vector3 b, Vector3 c) {
			A = a;
			B = b;
			C = c;
		}
	}
}