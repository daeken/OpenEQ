using System.Collections.Generic;
using System.Numerics;

namespace OpenEQ.Engine {
	public class AniModel {
		public readonly List<AnimatedMesh> Meshes = new List<AnimatedMesh>();

		public void Add(AnimatedMesh mesh) => Meshes.Add(mesh);

		public void Draw(Matrix4x4 projView, bool forward) => Meshes.ForEach(mesh => mesh.Draw(projView, forward));
	}
}