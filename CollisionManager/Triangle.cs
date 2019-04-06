using System.Collections.Generic;
using System.Numerics;
using OpenEQ.Common;

namespace CollisionManager {
	public struct Triangle {
		const float Epsilon = 0.00001f;
		
		public Vector3 A, B, C;
		public Vector3 Normal, Center;

		public Vector3[] AsArray => new[] { A, B, C };
		
		public AABB BoundingBox {
			get {
				var min = Vector3.Min(A, Vector3.Min(B, C));
				return new AABB(
					min,
					Vector3.Max(A, Vector3.Max(B, C)) - min
				);
			}
		}

		public Triangle(Vector3 a, Vector3 b, Vector3 c) {
			A = a;
			B = b;
			C = c;
			Normal = Vector3.Cross(b - a, c - a).Normalized();
			Center = (a + b + c) / 3;
		}

		// Moller-Trumbore
		public (Vector3, float)? FindIntersection(Vector3 origin, Vector3 direction) {
			var edge1 = B - A;
			var edge2 = C - A;
			var h = Vector3.Cross(direction, edge2);
			var a = Vector3.Dot(edge1, h);
			if(a < Epsilon) return null; // If it's < Epsilon but > -Epsilon, this is a miss; < -Epsilon means hitting a triangle on the opposite face
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

		public IEnumerable<Triangle> Split(Plane plane) {
			var d1 = Vector3.Dot(A, plane.Normal) - plane.Distance;
			var d2 = Vector3.Dot(B, plane.Normal) - plane.Distance;
			var d3 = Vector3.Dot(C, plane.Normal) - plane.Distance;

			if(d1 * d2 < 0) return Slice(A, B, C, d1, d2, d3);
			if(d1 * d3 < 0) return Slice(C, A, B, d3, d1, d2);
			if(d2 * d3 < 0) return Slice(B, C, A, d2, d3, d1);
			return new[] { this };
		}

		static IEnumerable<Triangle> Slice(Vector3 a, Vector3 b, Vector3 c, float d1, float d2, float d3) {
			var ab = a + d1 / (d1 - d2) * (b - a);
			if(d1 < 0) {
				if(d3 < 0) {
					var bc = b + d2 / (d2 - d3) * (c - b);
					yield return new Triangle(b, bc, ab);
					yield return new Triangle(bc, c, a);
					yield return new Triangle(ab, bc, a);
				} else {
					var ac = a + d1 / (d1 - d3) * (c - a);
					yield return new Triangle(a, ab, ac);
					yield return new Triangle(ab, b, c);
					yield return new Triangle(ac, ab, c);
				}
			} else {
				if(d3 < 0) {
					var ac = a + d1 / (d1 - d3) * (c - a);
					yield return new Triangle(a, ab, ac);
					yield return new Triangle(ac, ab, b);
					yield return new Triangle(b, c, ac);
				} else {
					var bc = b + d2 / (d2 - d3) * (c - b);
					yield return new Triangle(b, bc, ab);
					yield return new Triangle(a, ab, bc);
					yield return new Triangle(c, a, bc);
				}
			}
		}

		public override string ToString() => $"Triangle({A}, {B}, {C}, ({Normal}))";
	}
}