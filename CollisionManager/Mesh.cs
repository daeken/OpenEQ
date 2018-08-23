using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using static System.MathF;

namespace CollisionManager {
	public class Mesh {
		public readonly AABB BoundingBox;
		public readonly IReadOnlyList<Triangle> Triangles;
		
		public Mesh(IEnumerable<Triangle> triangles, bool skipBounding = false) {
			Triangles = triangles.ToList();
			if(Triangles.Count == 0 || skipBounding) return;
			var min = Triangles.First().A;
			var max = min;
			foreach(var point in Triangles.Select(x => x.AsArray).SelectMany(x => x)) {
				min = new Vector3(Min(min.X, point.X), Min(min.Y, point.Y), Min(min.Z, point.Z));
				max = new Vector3(Max(max.X, point.X), Max(max.Y, point.Y), Max(max.Z, point.Z));
			}
			BoundingBox = new AABB(min, max - min);
		}
		
		public Mesh WithBounding => new Mesh(Triangles);
		
		public static Mesh operator+(Mesh left, Mesh right) => new Mesh(left.Triangles.Concat(right.Triangles), skipBounding: true);
	}
}