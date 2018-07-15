using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Numerics;
using OpenEQ.Common;
using static System.MathF;

namespace OpenEQ.Engine {
	public static class Globals {
		public static Matrix4x4 ProjectionMat;
		public static readonly FpsCamera Camera = new FpsCamera(vec3());
		
		public readonly static Stopwatch Stopwatch = new Stopwatch();
		public static float Time => Stopwatch.ElapsedMilliseconds / 1000f;
		
		public static Vector2 vec2() => new Vector2();
		public static Vector2 vec2(float v) => new Vector2(v);
		public static Vector2 vec2(float x, float y) => new Vector2(x, y);

		public static Vector3 vec3() => new Vector3();
		public static Vector3 vec3(float v) => new Vector3(v);
		public static Vector3 vec3(float x, float y, float z) => new Vector3(x, y, z);

		public static Vector4 vec4() => new Vector4();
		public static Vector4 vec4(float v) => new Vector4(v);
		public static Vector4 vec4(Vector3 xyz, float w) => new Vector4(xyz.X, xyz.Y, xyz.Z, w);
		public static Vector4 vec4(float x, float y, float z, float w) => new Vector4(x, y, z, w);

		public static float clamp(float x, float min, float max) => Min(Max(x, min), max);
		public static float fract(float x) => x - Floor(x);

		public static Vector3 floor(Vector3 x) => vec3(Floor(x.X), Floor(x.Y), Floor(x.Z));

		public static float sign(float x) => x >= 0.0 ? 1 : -1;
		public static Vector2 sign(Vector2 x) => vec2(sign(x.X), sign(x.Y));
		public static Vector3 sign(Vector3 x) => vec3(sign(x.X), sign(x.Y), sign(x.Z));

		public static float min(float a, float b) => Min(a, b);
		public static Vector2 min(Vector2 a, Vector2 b) => vec2(Min(a.X, b.X), Min(a.Y, b.Y));
		public static Vector3 min(Vector3 a, Vector3 b) => vec3(Min(a.X, b.X), Min(a.Y, b.Y), Min(a.Z, b.Z));
		public static float max(float a, float b) => Max(a, b);
		public static Vector2 max(Vector2 a, Vector2 b) => vec2(Max(a.X, b.X), Max(a.Y, b.Y));
		public static Vector3 max(Vector3 a, Vector3 b) => vec3(Max(a.X, b.X), Max(a.Y, b.Y), Max(a.Z, b.Z));

		public static float abs(float v) => Abs(v);
		public static Vector2 abs(Vector2 v) => vec2(Abs(v.X), Abs(v.Y));
		public static Vector3 abs(Vector3 v) => vec3(Abs(v.X), Abs(v.Y), Abs(v.Z));

		public static float sqrt(float v) => Sqrt(v);
		public static Vector2 sqrt(Vector2 v) => vec2(Sqrt(v.X), Sqrt(v.Y));
		public static Vector3 sqrt(Vector3 v) => vec3(Sqrt(v.X), Sqrt(v.Y), Sqrt(v.Z));

		public static float sq(float v) => v * v;
		public static Vector2 sq(Vector2 v) => vec2(v.X * v.X, v.Y * v.Y);
		public static Vector3 sq(Vector3 v) => vec3(v.X * v.X, v.Y * v.Y, v.Z * v.Z);

		public static float mix(float a, float b, float x) => (b - a) * x + a;
		public static Vector3 mix(Vector3 a, Vector3 b, float x) => (b - a) * x + a;

		public static float sin(float v) => Sin(v);
		public static Vector2 sin(Vector2 v) => vec2(Sin(v.X), Sin(v.Y));
		public static Vector3 sin(Vector3 v) => vec3(Sin(v.X), Sin(v.Y), Sin(v.Z));
		
		public static LazyProperty<T> lazy<T>(Func<T> func) => new LazyProperty<T>(func);

		static readonly Dictionary<string, List<float>> ProfileRunning = new Dictionary<string, List<float>>();
		public static void Profile(string name, Action func) {
#if DEBUG
			var pre = Stopwatch.ElapsedTicks;
			func();
			var post = Stopwatch.ElapsedTicks;
			var ms = (float) (post - pre) / System.Diagnostics.Stopwatch.Frequency * 1000;
			var pr = ProfileRunning.ContainsKey(name)
				? ProfileRunning[name]
				: ProfileRunning[name] = new List<float>();
			pr.Add(ms);
			Console.WriteLine($"{name} took {Round(ms, 2)} ms (average {Round(pr.Average(), 2)} over {pr.Count} samples)");
#else
			func();
#endif
		}
	}
}