using System.Numerics;

namespace CollisionManager {
	public class CollisionHelper {
		public readonly Octree Octree;

		public CollisionHelper(Octree octree) {
			Octree = octree;
		}

		public (Triangle, Vector3)? FindIntersection(Vector3 origin, Vector3 direction) =>
			Octree.FindIntersection(origin, direction);
	}
}