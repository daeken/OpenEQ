using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using OpenEQ.Common;
using static System.Math;

namespace OpenEQ.Engine {
	public static class Globals {
		public static Mat4 ProjectionMat;
		public static readonly FpsCamera Camera = new FpsCamera(vec3());
		
		public readonly static Stopwatch Stopwatch = new Stopwatch();
		public static double Time => Stopwatch.ElapsedMilliseconds / 1000.0;
		
		public static Vec2 vec2() => new Vec2();
		public static Vec2 vec2(float v) => new Vec2(v);
		public static Vec2 vec2(double v) => new Vec2(v);
		public static Vec2 vec2(float x, float y) => new Vec2(x, y);
		public static Vec2 vec2(double x, double y) => new Vec2(x, y);

		public static Vec3 vec3() => new Vec3();
		public static Vec3 vec3(float v) => new Vec3(v);
		public static Vec3 vec3(double v) => new Vec3(v);
		public static Vec3 vec3(float x, float y, float z) => new Vec3(x, y, z);
		public static Vec3 vec3(double x, double y, double z) => new Vec3(x, y, z);

		public static Vec4 vec4() => new Vec4();
		public static Vec4 vec4(float v) => new Vec4(v);
		public static Vec4 vec4(double v) => new Vec4(v);
		public static Vec4 vec4(Vec3 xyz, float w) => new Vec4(xyz.X, xyz.Y, xyz.Z, w);
		public static Vec4 vec4(Vec3 xyz, double w) => new Vec4(xyz.X, xyz.Y, xyz.Z, w);
		public static Vec4 vec4(float x, float y, float z, float w) => new Vec4(x, y, z, w);
		public static Vec4 vec4(double x, double y, double z, double w) => new Vec4(x, y, z, w);

		public static double clamp(double x, double min, double max) => Min(Max(x, min), max);
		public static double fract(double x) => x - Floor(x);

		public static Vec3 floor(Vec3 x) => vec3(Floor(x.X), Floor(x.Y), Floor(x.Z));

		public static double sign(double x) => x >= 0.0 ? 1 : -1;
		public static Vec2 sign(Vec2 x) => vec2(sign(x.X), sign(x.Y));
		public static Vec3 sign(Vec3 x) => vec3(sign(x.X), sign(x.Y), sign(x.Z));

		public static double min(double a, double b) => Min(a, b);
		public static Vec2 min(Vec2 a, Vec2 b) => vec2(Min(a.X, b.X), Min(a.Y, b.Y));
		public static Vec3 min(Vec3 a, Vec3 b) => vec3(Min(a.X, b.X), Min(a.Y, b.Y), Min(a.Z, b.Z));
		public static double max(double a, double b) => Max(a, b);
		public static Vec2 max(Vec2 a, Vec2 b) => vec2(Max(a.X, b.X), Max(a.Y, b.Y));
		public static Vec3 max(Vec3 a, Vec3 b) => vec3(Max(a.X, b.X), Max(a.Y, b.Y), Max(a.Z, b.Z));

		public static double abs(double v) => Abs(v);
		public static Vec2 abs(Vec2 v) => vec2(Abs(v.X), Abs(v.Y));
		public static Vec3 abs(Vec3 v) => vec3(Abs(v.X), Abs(v.Y), Abs(v.Z));

		public static double sqrt(double v) => Sqrt(v);
		public static Vec2 sqrt(Vec2 v) => vec2(Sqrt(v.X), Sqrt(v.Y));
		public static Vec3 sqrt(Vec3 v) => vec3(Sqrt(v.X), Sqrt(v.Y), Sqrt(v.Z));

		public static double sq(double v) => v * v;
		public static Vec2 sq(Vec2 v) => vec2(v.X * v.X, v.Y * v.Y);
		public static Vec3 sq(Vec3 v) => vec3(v.X * v.X, v.Y * v.Y, v.Z * v.Z);

		public static double mix(double a, double b, double x) => (b - a) * x + a;
		public static Vec3 mix(Vec3 a, Vec3 b, double x) => (b - a) * x + a;

		public static double sin(double v) => Sin(v);
		public static Vec2 sin(Vec2 v) => vec2(Sin(v.X), Sin(v.Y));
		public static Vec3 sin(Vec3 v) => vec3(Sin(v.X), Sin(v.Y), Sin(v.Z));
		
		public static LazyProperty<T> lazy<T>(Func<T> func) => new LazyProperty<T>(func);

		static Dictionary<string, List<double>> ProfileRunning = new Dictionary<string, List<double>>();
		public static void Profile(string name, Action func) {
#if DEBUG
			var pre = Stopwatch.ElapsedTicks;
			func();
			var post = Stopwatch.ElapsedTicks;
			var ms = (double) (post - pre) / System.Diagnostics.Stopwatch.Frequency * 1000;
			var pr = ProfileRunning.ContainsKey(name)
				? ProfileRunning[name]
				: ProfileRunning[name] = new List<double>();
			pr.Add(ms);
			Console.WriteLine($"{name} took {Round(ms, 2)} ms (average {Round(pr.Average(), 2)} over {pr.Count} samples)");
#else
			func();
#endif
		}
	}
}