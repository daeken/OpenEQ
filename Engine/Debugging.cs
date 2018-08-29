using System.Collections.Generic;
using System.Numerics;

namespace OpenEQ.Engine {
	public static class Debugging {
		public static readonly List<Wireframe> Wireframes = new List<Wireframe>();

		public static void Add(Wireframe wireframe) => Wireframes.Add(wireframe);

		public static void Draw(Matrix4x4 projView) => Wireframes.ForEach(wireframe => wireframe.Draw(projView));
	}
}