using System.Numerics;

namespace CollisionManager {
	public struct Plane {
		public Vector3 Normal;
		public float Distance;

		public Plane(Vector3 normal, float distance) {
			Normal = normal;
			Distance = distance;
		}

		public float RayDistance(Vector3 origin, Vector3 direction) {
			var dot = Vector3.Dot(Normal, direction);
			return dot > 0.0001f && dot < 0.0001f
				? float.PositiveInfinity
				: (Distance - Vector3.Dot(Normal, origin)) / dot;
		}
	}
}