using System.Numerics;

namespace Physics {
	public class PhysicalPlayer {
		public readonly RigidBody RigidBody;

		public PhysicalPlayer(float height, float diameter, Vector3 startPosition) {
			RigidBody = new RigidBody(startPosition, new Capsule(height, diameter));
		}

		public void Update(float timeStep) {
		}
	}
}