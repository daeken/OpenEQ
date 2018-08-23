using System;
using System.Numerics;

namespace CollisionManager {
	public struct Triangle {
		const float Epsilon = 0.00001f;
		
		public Vector3 A, B, C;
		public Vector3 Normal;

		public Vector3[] AsArray => new[] { A, B, C };

		public Triangle(Vector3 a, Vector3 b, Vector3 c) {
			A = a;
			B = b;
			C = c;
			Normal = Vector3.Cross(b - a, c - a);
		}

		// Moller-Trumbore
		public (Vector3, float)? FindIntersection(Vector3 origin, Vector3 direction) {
			var edge1 = B - A;
			var edge2 = C - A;
			var h = Vector3.Cross(direction, edge2);
			var a = Vector3.Dot(edge1, h);
			if(a > -Epsilon && a < Epsilon) return null; // If it's < Epsilon, it's facing away from us; we can cull later if needed
			var f = 1 / a;
			var s = origin - A;
			var u = f * Vector3.Dot(s, h);
			if(u < 0 || u > 1) return null;
			var q = Vector3.Cross(s, edge1);
			var v = f * Vector3.Dot(direction, q);
			if(v < 0 || u + v > 1) return null;
			var t = f * Vector3.Dot(edge2, q);
			if(t <= Epsilon) return null;
			return (origin + direction * t, t);
		}
	}
}