using System;

namespace OpenEQ.Common {
	public struct Vec2 {
		public static readonly Vec2 Zero = new Vec2();
		public double X, Y;
		public double Length => Math.Sqrt(X * X + Y * Y);
		public Vec2 Normalized {
			get {
				var len = Length;
				if(len == 0)
					return new Vec2();
				return new Vec2(X / len, Y / len);
			}
		}
		
		public double[] ToArray() => new[] { X, Y };
		public Vec2 Abs => new Vec2(Math.Abs(X), Math.Abs(Y));
		public Vec2 Exp => new Vec2(Math.Exp(X), Math.Exp(Y));
		public Vec2 Log => new Vec2(Math.Log(X), Math.Log(Y));
		public Vec2 Log2 => new Vec2(Math.Log(X, 2), Math.Log(Y, 2));
		public Vec2 Sqrt => new Vec2(Math.Sqrt(X), Math.Sqrt(Y));
		public Vec2 InverseSqrt => new Vec2(1 / Math.Sqrt(X), 1 / Math.Sqrt(Y));

		public Vec2(double v) {
			X = Y = v;
		}

		public Vec2(double x, double y) {
			X = x;
			Y = y;
		}

		public Vec2(double[] v) {
			X = v[0];
			Y = v[1];
		}

		public Vec2(int[] v) {
			X = v[0];
			Y = v[1];
		}

		public static Vec2 operator +(Vec2 left, double right) {
			return new Vec2(left.X + right, left.Y + right);
		}
		public static Vec2 operator +(Vec2 left, Vec2 right) {
			return new Vec2(left.X + right.X, left.Y + right.Y);
		}

		public static Vec2 operator -(Vec2 left, double right) {
			return new Vec2(left.X - right, left.Y - right);
		}
		public static Vec2 operator -(Vec2 left, Vec2 right) {
			return new Vec2(left.X - right.X, left.Y - right.Y);
		}

		public static Vec2 operator *(Vec2 left, double right) {
			return new Vec2(left.X * right, left.Y * right);
		}
		public static Vec2 operator *(Vec2 left, Vec2 right) {
			return new Vec2(left.X * right.X, left.Y * right.Y);
		}

		public static Vec2 operator /(Vec2 left, double right) {
			return new Vec2(left.X / right, left.Y / right);
		}
		public static Vec2 operator /(Vec2 left, Vec2 right) {
			return new Vec2(left.X / right.X, left.Y / right.Y);
		}

		public static double operator %(Vec2 left, Vec2 right) {
			return left.Dot(right);
		}

		public static Vec2 operator -(Vec2 v) {
			return new Vec2(-v.X, -v.Y);
		}

		public double Dot() => X * X + Y * Y;
		
		public double Dot(Vec2 right) {
			var temp = this * right;
			return temp.X + temp.Y;
		}

		public Vec2 ToPolar() => new Vec2(Math.Atan2(Y, X), Length);
		public Vec2 ToCartesian() => new Vec2(Y * Math.Cos(X), Y * Math.Sin(X));

		public override bool Equals(object obj) {
			if(obj is Vec2 b)
				return (int) (X * 100000 + .5) == (int) (b.X * 100000 + .5) && (int) (Y * 100000 + .5) == (int) (b.Y * 100000 + .5);
			return base.Equals(obj);
		}

		public override int GetHashCode() =>
			((int) (X * 100000 + .5)).GetHashCode() * 67 + ((int) (Y * 100000 + .5)).GetHashCode() * 17;

		public override string ToString() {
			return $"Vec2[ {X} {Y} ]";
		}
	}

	public struct Vec3 {
		public static readonly Vec3 Zero = new Vec3();
		public static readonly Vec3 One = new Vec3(1, 1, 1);
		public readonly double X, Y, Z;
		public double Length => Math.Sqrt(X * X + Y * Y + Z * Z);
		public Vec3 Normalized {
			get {
				var len = Length;
				if(len == 0)
					return new Vec3();
				return new Vec3(X / len, Y / len, Z / len);
			}
		}

		public double[] ToArray() => new[] { X, Y, Z };
		public Vec3 Abs => new Vec3(Math.Abs(X), Math.Abs(Y), Math.Abs(Z));
		public Vec3 Exp => new Vec3(Math.Exp(X), Math.Exp(Y), Math.Exp(Z));
		public Vec3 Log => new Vec3(Math.Log(X), Math.Log(Y), Math.Log(Z));
		public Vec3 Log2 => new Vec3(Math.Log(X, 2), Math.Log(Y, 2), Math.Log(Z, 2));
		public Vec3 Sqrt => new Vec3(Math.Sqrt(X), Math.Sqrt(Y), Math.Sqrt(Z));
		public Vec3 InverseSqrt => new Vec3(1 / Math.Sqrt(X), 1 / Math.Sqrt(Y), 1 / Math.Sqrt(Z));

		public Vec2 XX => new Vec2(X, X);
		public Vec2 XY => new Vec2(X, Y);
		public Vec2 XZ => new Vec2(X, Z);

		public Vec2 YX => new Vec2(Y, X);
		public Vec2 YY => new Vec2(Y, Y);
		public Vec2 YZ => new Vec2(Y, Z);

		public Vec2 ZX => new Vec2(Z, X);
		public Vec2 ZY => new Vec2(Z, Y);
		public Vec2 ZZ => new Vec2(Z, Z);

		public Vec3 XXX => new Vec3(X, X, X);
		public Vec3 XXY => new Vec3(X, X, Y);
		public Vec3 XXZ => new Vec3(X, X, Z);
		public Vec3 XYX => new Vec3(X, Y, X);
		public Vec3 XYY => new Vec3(X, Y, Y);
		public Vec3 XYZ => this;
		public Vec3 XZX => new Vec3(X, Z, X);
		public Vec3 XZY => new Vec3(X, Z, Y);
		public Vec3 XZZ => new Vec3(X, Z, Z);

		public Vec3 YXX => new Vec3(Y, X, X);
		public Vec3 YXY => new Vec3(Y, X, Y);
		public Vec3 YXZ => new Vec3(Y, X, Z);
		public Vec3 YYX => new Vec3(Y, Y, X);
		public Vec3 YYY => new Vec3(Y, Y, Y);
		public Vec3 YYZ => new Vec3(Y, Y, Z);
		public Vec3 YZX => new Vec3(Y, Z, X);
		public Vec3 YZY => new Vec3(Y, Z, Y);
		public Vec3 YZZ => new Vec3(Y, Z, Z);

		public Vec3 ZXX => new Vec3(Z, X, X);
		public Vec3 ZXY => new Vec3(Z, X, Y);
		public Vec3 ZXZ => new Vec3(Z, X, Z);
		public Vec3 ZYX => new Vec3(Z, Y, X);
		public Vec3 ZYY => new Vec3(Z, Y, Y);
		public Vec3 ZYZ => new Vec3(Z, Y, Z);
		public Vec3 ZZX => new Vec3(Z, Z, X);
		public Vec3 ZZY => new Vec3(Z, Z, Y);
		public Vec3 ZZZ => new Vec3(Z, Z, Z);

		public Vec3(double v) {
			X = Y = Z = v;
		}

		public Vec3(double x, double y, double z) {
			X = x;
			Y = y;
			Z = z;
		}

		public Vec3(double[] v) {
			X = v[0];
			Y = v[1];
			Z = v[2];
		}

		public Vec3(int[] v) {
			X = v[0];
			Y = v[1];
			Z = v[2];
		}

		public static Vec3 operator +(Vec3 left, double right) {
			return new Vec3(left.X + right, left.Y + right, left.Z + right);
		}
		public static Vec3 operator +(Vec3 left, Vec3 right) {
			return new Vec3(left.X + right.X, left.Y + right.Y, left.Z + right.Z);
		}

		public static Vec3 operator -(Vec3 left, double right) {
			return new Vec3(left.X - right, left.Y - right, left.Z - right);
		}
		public static Vec3 operator -(Vec3 left, Vec3 right) {
			return new Vec3(left.X - right.X, left.Y - right.Y, left.Z - right.Z);
		}

		public static Vec3 operator *(Vec3 left, double right) {
			return new Vec3(left.X * right, left.Y * right, left.Z * right);
		}
		public static Vec3 operator *(Vec3 left, Vec3 right) {
			return new Vec3(left.X * right.X, left.Y * right.Y, left.Z * right.Z);
		}

		public static Vec3 operator /(Vec3 left, double right) {
			return new Vec3(left.X / right, left.Y / right, left.Z / right);
		}
		public static Vec3 operator /(Vec3 left, Vec3 right) {
			return new Vec3(left.X / right.X, left.Y / right.Y, left.Z / right.Z);
		}

		public static double operator %(Vec3 left, Vec3 right) => left.Dot(right);

		public static Vec3 operator ^(Vec3 left, Vec3 right) => left.Cross(right);

		public static Vec3 operator -(Vec3 v) => new Vec3(-v.X, -v.Y, -v.Z);

		public static bool operator <=(Vec3 left, Vec3 right) => left.X <= right.X && left.Y <= right.Y && left.Z <= right.Z;
		public static bool operator >=(Vec3 left, Vec3 right) => left.X >= right.X && left.Y >= right.Y && left.Z >= right.Z;

		public double Dot() => X * X + Y * Y + Z * Z;
		
		public double Dot(Vec3 right) {
			var temp = this * right;
			return temp.X + temp.Y + temp.Z;
		}

		public Vec3 Cross(Vec3 right) => new Vec3(Y * right.Z - Z * right.Y, Z * right.X - X * right.Z, X * right.Y - Y * right.X);

		public static double Mod(double x, double y) => x - y * Math.Floor(x / y);
		public Vec3 Mod(Vec3 right) =>
			new Vec3(Mod(X, right.X), Mod(Y, right.Y), Mod(Z, right.Z));

		public (Vec3, Vec3) Sort(Vec3 right) {
			if(right.X < X) return (right, this);
			if(right.X > X) return (this, right);
			if(right.Y < Y) return (right, this);
			if(right.Y > Y) return (this, right);
			if(right.Z < Z) return (right, this);
			return (this, right);
		}

		public override string ToString() {
			return $"Vec3[ {X} {Y} {Z} ]";
		}

		public override bool Equals(object obj) {
			if(obj is Vec3 b)
				return (int) (X * 100000 + .5) == (int) (b.X * 100000 + .5) && (int) (Y * 100000 + .5) == (int) (b.Y * 100000 + .5) && (int) (Z * 100000 + .5) == (int) (b.Z * 100000 + .5);
			return base.Equals(obj);
		}

		public override int GetHashCode() =>
			((int) (X * 100000 + .5)).GetHashCode() * 67 + ((int) (Y * 100000 + .5)).GetHashCode() * 17 + ((int) (Z * 100000 + .5)).GetHashCode();

		public double this[int i] => i == 0 ? X : (i == 1 ? Y : Z);
	}

	public struct Vec4 {
		public double X, Y, Z, W;
		public double Length => Math.Sqrt(X * X + Y * Y + Z * Z + W * W);
		public Vec4 Normalized {
			get {
				var len = Length;
				if(len == 0)
					return new Vec4();
				return new Vec4(X / len, Y / len, Z / len, W / len);
			}
		}
		
		public double[] ToArray() => new[] { X, Y, Z, W };
		public Vec4 Abs => new Vec4(Math.Abs(X), Math.Abs(Y), Math.Abs(Z), Math.Abs(W));
		public Vec4 Exp => new Vec4(Math.Exp(X), Math.Exp(Y), Math.Exp(Z), Math.Exp(W));
		public Vec4 Log => new Vec4(Math.Log(X), Math.Log(Y), Math.Log(Z), Math.Log(W));
		public Vec4 Log2 => new Vec4(Math.Log(X, 2), Math.Log(Y, 2), Math.Log(Z, 2), Math.Log(W, 2));
		public Vec4 Sqrt => new Vec4(Math.Sqrt(X), Math.Sqrt(Y), Math.Sqrt(Z), Math.Sqrt(W));
		public Vec4 InverseSqrt => new Vec4(1 / Math.Sqrt(X), 1 / Math.Sqrt(Y), 1 / Math.Sqrt(Z), 1 / Math.Sqrt(W));
		
		public Vec2 XY => new Vec2(X, Y);
		public Vec3 XYZ => new Vec3(X, Y, Z);

		public Vec4(double v) {
			X = Y = Z = W = v;
		}

		public Vec4(double x, double y, double z, double w) {
			X = x;
			Y = y;
			Z = z;
			W = w;
		}

		public Vec4(double[] v) {
			X = v[0];
			Y = v[1];
			Z = v[2];
			W = v[3];
		}

		public Vec4(int[] v) {
			X = v[0];
			Y = v[1];
			Z = v[2];
			W = v[3];
		}

		public static Vec4 operator +(Vec4 left, double right) {
			return new Vec4(left.X + right, left.Y + right, left.Z + right, left.W + right);
		}
		public static Vec4 operator +(Vec4 left, Vec4 right) {
			return new Vec4(left.X + right.X, left.Y + right.Y, left.Z + right.Z, left.W + right.W);
		}

		public static Vec4 operator -(Vec4 left, double right) {
			return new Vec4(left.X - right, left.Y - right, left.Z - right, left.W - right);
		}
		public static Vec4 operator -(Vec4 left, Vec4 right) {
			return new Vec4(left.X - right.X, left.Y - right.Y, left.Z - right.Z, left.W - right.W);
		}

		public static Vec4 operator *(Vec4 left, double right) {
			return new Vec4(left.X * right, left.Y * right, left.Z * right, left.W * right);
		}
		public static Vec4 operator *(Vec4 left, Vec4 right) {
			return new Vec4(left.X * right.X, left.Y * right.Y, left.Z * right.Z, left.W * right.W);
		}

		public static Vec4 operator /(Vec4 left, double right) {
			return new Vec4(left.X / right, left.Y / right, left.Z / right, left.W / right);
		}
		public static Vec4 operator /(Vec4 left, Vec4 right) {
			return new Vec4(left.X / right.X, left.Y / right.Y, left.Z / right.Z, left.W / right.W);
		}

		public static double operator %(Vec4 left, Vec4 right) {
			return left.Dot(right);
		}

		public static Vec4 operator -(Vec4 v) {
			return new Vec4(-v.X, -v.Y, -v.Z, -v.W);
		}

		public double Dot() => X * X + Y * Y + Z * Z + W * W;
		
		public double Dot(Vec4 right) {
			var temp = this * right;
			return temp.X + temp.Y + temp.Z + temp.W;
		}

		public override string ToString() {
			return $"Vec4[ {X} {Y} {Z} {W} ]";
		}
	}
}