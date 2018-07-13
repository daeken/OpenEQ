using System;

namespace OpenEQ.Common {
	public struct Quaternion {
		public double X, Y, Z, W;

		public double Length => Math.Sqrt(X * X + Y * Y + Z * Z + W * W);
		public Quaternion Normalized {
			get {
				var len = Length;
				if(len == 0)
					return new Quaternion();
				return new Quaternion(X / len, Y / len, Z / len, W / len);
			}
		}

		public Quaternion(double x, double y, double z, double w) {
			X = x;
			Y = y;
			Z = z;
			W = w;
		}

		public Mat4 ToMatrix() {
			return new Mat4(
				1 - 2 * Y * Y - 2 * Z * Z, 2 * X * Y - 2 * Z * W, 2 * X * Z + 2 * Y * W, 0, 
				2 * X * Y + 2 * Z * W, 1 - 2 * X * X - 2 * Z * Z, 2 * Y * Z - 2 * X * W, 0, 
				2 * X * Z - 2 * Y * W, 2 * Y * Z + 2 * X * W, 1 - 2 * X * X - 2 * Y * Y, 0, 
				0, 0, 0, 1
			);
		}

		public static Quaternion operator +(Quaternion left, double right) {
			return new Quaternion(left.X + right, left.Y + right, left.Z + right, left.W + right);
		}
		public static Quaternion operator +(Quaternion left, Quaternion right) {
			return new Quaternion(left.X + right.X, left.Y + right.Y, left.Z + right.Z, left.W + right.W);
		}

		public static Quaternion operator -(Quaternion left, double right) {
			return new Quaternion(left.X - right, left.Y - right, left.Z - right, left.W - right);
		}
		public static Quaternion operator -(Quaternion left, Quaternion right) {
			return new Quaternion(left.X - right.X, left.Y - right.Y, left.Z - right.Z, left.W - right.W);
		}

		public static Quaternion operator *(Quaternion left, double right) {
			return new Quaternion(left.X * right, left.Y * right, left.Z * right, left.W * right);
		}
		public static Quaternion operator *(Quaternion left, Quaternion right) {
			return new Quaternion(
				left.X * right.W + left.Y * right.Z - left.Z * right.Y + left.W * right.X, 
				-left.X * right.Z + left.Y * right.W + left.Z * right.X + left.W * right.Y, 
				left.X * right.Y - left.Y * right.X + left.Z * right.W + left.W * right.Z, 
				-left.X * right.X - left.Y * right.Y - left.Z * right.Z + left.W * right.W
			);
		}

		public static Quaternion operator /(Quaternion left, double right) {
			return new Quaternion(left.X / right, left.Y / right, left.Z / right, left.W / right);
		}
		public static Quaternion operator /(Quaternion left, Quaternion right) {
			return new Quaternion(left.X / right.X, left.Y / right.Y, left.Z / right.Z, left.W / right.W);
		}

		public static Quaternion operator -(Quaternion left) {
			return new Quaternion(-left.X, -left.Y, -left.Z, -left.W);
		}

		public static double operator %(Quaternion left, Quaternion right) {
			return left.Dot(right);
		}

		public double Dot(Quaternion right) {
			return X * right.X + Y * right.Y + Z * right.Z + W * right.W;
		}
		
		public static Quaternion FromAxisAngle(Vec3 axis, double angle) {
			axis = axis * Math.Sin(angle / 2);
			return new Quaternion(axis.X, axis.Y, axis.Z, Math.Cos(angle / 2)).Normalized;
		}
	}
}