using System.Numerics;

namespace CollisionManager {
	public struct Plane {
		public Vector3 Normal;
		public float Distance;

		public Plane(Vector3 normal, float distance) {
			Normal = normal;
			Distance = distance;
		}
	}
}