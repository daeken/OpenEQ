using System.Numerics;

namespace Physics {
	public class RigidBody {
		public Vector3 Velocity, Position;
		public readonly ICollidable Collider;

		public RigidBody(Vector3 position, ICollidable collider) {
			Position = position;
			Collider = collider;
		}
	}
}