using System;
using System.Numerics;

namespace CollisionManager {
	public static class Extensions {
		public static Vector3 Select(this (Vector3 A, Vector3 B) v, int sa, int sb, int sc) {
			if(sa != 1 && sb != 1 && sc != 1) return v.A;
			if(sa == 1 && sb == 1 && sc == 1) return v.B;
			return new Vector3(
				sa == 1 ? v.B.X : v.A.X, 
				sb == 1 ? v.B.Y : v.A.Y, 
				sc == 1 ? v.B.Z : v.A.Z
			);
		}

		public static float ComponentMin(this Vector3 v) => MathF.Min(MathF.Min(v.X, v.Y), v.Z);
		public static float ComponentMax(this Vector3 v) => MathF.Max(MathF.Max(v.X, v.Y), v.Z);
	}
}