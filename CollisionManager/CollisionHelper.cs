using System.Collections.Generic;
using System.Linq;
using System.Numerics;

namespace CollisionManager {
	public class CollisionHelper {
		public readonly Octree Octree;

		public CollisionHelper(Octree octree) {
			Octree = octree;
		}

		public (Triangle, Vector3)? FindIntersection(Vector3 origin, Vector3 direction) =>
			Octree.FindIntersectionCustom(origin, direction);

		public (Triangle, Vector3)? FindIntersection(Vector3 origin, Vector3 direction, float spread) =>
			Octree.FindIntersectionCustom(origin, direction, spread);

		public (Triangle, Vector3)? FindFirstIntersection(IEnumerable<Vector3> origins, Vector3 direction) =>
			origins.Select(x => (x, FindIntersection(x, direction))).Where(x => x.Item2 != null)
				.Select(x => (x.Item2, ((x.Item2.Value.Item2 - x.Item1) * direction).LengthSquared()))
				.OrderBy(x => x.Item2).Select(x => x.Item1).FirstOrDefault();
	}
}