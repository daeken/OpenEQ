using System.Numerics;

namespace CollisionManager {
	public struct AABB {
		public readonly Vector3 Min, Max;
		public readonly Vector3 Size;
		public readonly Vector3 Center;

		public AABB(Vector3 min, Vector3 size) {
			Min = min;
			Max = min + size;
			Size = size;
			Center = min + size / 2;
		}

		// TODO: Make this handle cases where a portion of a triangle is within this AABB!
		public bool Contains(Triangle tri) =>
			Contains(tri.A) || Contains(tri.B) || Contains(tri.C);

		public bool Contains(Vector3 point) =>
			Min.X <= point.X && Min.Y <= point.Y && Min.Z <= point.Z && 
			Max.X >= point.X && Max.Y >= point.Y && Max.Z >= point.Z;

		public bool IntersectedBy(Vector3 origin, Vector3 direction) {
			if(Contains(origin)) return true;
			
			var tmin = (Min.X - origin.X) / direction.X;
			var tmax = (Max.X - origin.X) / direction.X;
			if(tmin > tmax) { var temp = tmin; tmin = tmax; tmax = temp; }
			
			var tymin = (Min.Y - origin.Y) / direction.Y;
			var tymax = (Max.Y - origin.Y) / direction.Y;
			if(tymin > tymax) { var temp = tymin; tymin = tymax; tymax = temp; }

			if(tmin > tymax || tymin > tmax) return false;

			if(tymin > tmin) tmin = tymin;
			if(tymax < tmax) tmax = tymax;
			
			var tzmin = (Min.Z - origin.Z) / direction.Z;
			var tzmax = (Max.Z - origin.Z) / direction.Z;
			if(tzmin > tzmax) { var temp = tzmin; tzmin = tzmax; tzmax = temp; }

			return tmin <= tzmax && tzmin <= tmax;
		}

		public override string ToString() => $"AABB(Min={Min}, Max={Max})";
	}
}