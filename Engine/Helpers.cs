using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using OpenEQ.Common;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public static class Helpers {
		public static (float[], uint[]) MakeSphereGeometry(int slices, int rings) {
			var vertices = new List<Vector3>();
			var index = 0;
			var grid = new List<List<int>>();
			for(var iy = 0; iy <= rings; ++iy) {
				var vrow = new List<int>();
				var v = iy / (float) rings;
				for(var ix = 0; ix <= slices; ++ix) {
					var u = ix / (float) slices;

					var vert = vec3(
						-MathF.Cos(u * MathF.PI * 2) * MathF.Sin(v * MathF.PI),
						MathF.Cos(v * MathF.PI),
						MathF.Sin(u * MathF.PI * 2) * MathF.Sin(v * MathF.PI)
					);

					vertices.Add(vert);
					vrow.Add(index++);
				}

				grid.Add(vrow);
			}

			var indices = new List<(int, int, int)>();

			for(var iy = 0; iy < rings; ++iy) {
				for(var ix = 0; ix < slices; ++ix) {
					var a = grid[iy][ix + 1];
					var b = grid[iy][ix];
					var c = grid[iy + 1][ix];
					var d = grid[iy + 1][ix + 1];

					if(iy > 0) indices.Add((a, b, d));
					if(iy < rings - 1) indices.Add((b, c, d));
				}
			}

			var buf = new List<float>();
			for(var i = 0; i < vertices.Count; ++i) {
				buf.Add(vertices[i].X);
				buf.Add(vertices[i].Y);
				buf.Add(vertices[i].Z);
			}

			return (buf.ToArray(), indices.Select(x => new[] { (uint) x.Item1, (uint) x.Item2, (uint) x.Item3 }).SelectMany(x => x).ToArray());
		}
	}
}