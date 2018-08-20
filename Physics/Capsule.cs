namespace Physics {
	public class Capsule : ICollidable {
		public readonly float Height, Diameter;

		public Capsule(float height, float diameter) {
			Height = height;
			Diameter = diameter;
		}
	}
}