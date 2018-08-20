using System.Numerics;

namespace Physics {
	public struct AABB : ICollidable {
		public readonly Vector3 Min, Max;
		public readonly Vector3 Size;

		public AABB(Vector3 min, Vector3 size) {
			Min = min;
			Max = min + size;
			Size = size;
		}

		// TODO: Make this handle cases where a portion of a triangle is within this AABB!
		public bool Contains(Triangle tri) =>
			Contains(tri.A) || Contains(tri.B) || Contains(tri.C);

		public bool Contains(Vector3 point) =>
			Min.X <= point.X && Min.Y <= point.Y && Min.Z <= point.Z && 
			Max.X >= point.X && Max.Y >= point.Y && Max.Z >= point.Z;

		public override string ToString() => $"AABB(Min={Min}, Max={Max})";
	}
}