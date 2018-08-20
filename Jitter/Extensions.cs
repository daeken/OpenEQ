using System;
using System.Numerics;
using Jitter.LinearMath;

namespace Jitter {
	public static class Extensions {
		public const float Epsilon = 1.192092896e-012f;

		public static Vector3 Transform(this Vector3 position, JMatrix matrix) => position.Transform(ref matrix);

		public static Vector3 Transform(this Vector3 position, ref JMatrix matrix) {
			position.Transform(ref matrix, out var ret);
			return ret;
		}

		public static void Transform(this Vector3 position, ref JMatrix matrix, out Vector3 result) {
			var num0 = position.X * matrix.M11 + position.Y * matrix.M21 + position.Z * matrix.M31;
			var num1 = position.X * matrix.M12 + position.Y * matrix.M22 + position.Z * matrix.M32;
			var num2 = position.X * matrix.M13 + position.Y * matrix.M23 + position.Z * matrix.M33;

			result.X = num0;
			result.Y = num1;
			result.Z = num2;
		}

		public static Vector3 TransposedTransform(this Vector3 position, ref JMatrix matrix) {
			position.TransposedTransform(ref matrix, out var ret);
			return ret;
		}

		public static void TransposedTransform(this Vector3 position, ref JMatrix matrix, out Vector3 result) {
			var num0 = position.X * matrix.M11 + position.Y * matrix.M12 + position.Z * matrix.M13;
			var num1 = position.X * matrix.M21 + position.Y * matrix.M22 + position.Z * matrix.M23;
			var num2 = position.X * matrix.M31 + position.Y * matrix.M32 + position.Z * matrix.M33;

			result.X = num0;
			result.Y = num1;
			result.Z = num2;
		}

		public static void Normalize(this ref Vector3 vector) {
			vector = Vector3.Normalize(vector);
		}

		public static void Set(this Vector3 vec, float x, float y, float z) {
			vec.X = x;
			vec.Y = y;
			vec.Z = z;
		}

		public static void Min(ref Vector3 a, ref Vector3 b, out Vector3 ret) {
			ret = new Vector3(MathF.Min(a.X, b.X), MathF.Min(a.Y, b.Y), MathF.Min(a.Z, b.Z));
		}

		public static void Max(ref Vector3 a, ref Vector3 b, out Vector3 ret) {
			ret = new Vector3(MathF.Max(a.X, b.X), MathF.Max(a.Y, b.Y), MathF.Max(a.Z, b.Z));
		}

		public static bool IsNearlyZero(this Vector3 vec) =>
			vec.X * vec.X + vec.Y * vec.Y + vec.Z * vec.Z < Epsilon * Epsilon;

		public static void Swap(ref Vector3 a, ref Vector3 b) {
			var temp = a;
			a = b;
			b = temp;
		}
	}
}