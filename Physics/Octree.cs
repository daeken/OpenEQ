using System.Collections.Generic;
using System.Linq;
using System.Numerics;

namespace Physics {
	public class Octree {
		/*
		 * A -- min, min, min
		 * B -- max, min, min
		 * C -- min, max, min
		 * D -- max, max, min
		 * E -- min, min, max
		 * F -- max, min, max
		 * G -- min, max, max
		 * H -- max, max, max
		 */
		public readonly Octree A, B, C, D, E, F, G, H;

		public Mesh Leaf;

		public Octree(Mesh mesh, int maxTrisPerLeaf) {
			if(mesh.Triangles.Count <= maxTrisPerLeaf) {
				Leaf = mesh;
				return;
			}

			Mesh BuildSub(AABB bb) =>
				new Mesh(mesh.Triangles.Where(bb.Contains));

			var c = mesh.BoundingBox.Min;
			var hs = mesh.BoundingBox.Size / 2;
			var nodes = new[] {
				new AABB(c, hs), 
				new AABB(new Vector3(c.X + hs.X, c.Y, c.Z), hs), 
				new AABB(new Vector3(c.X, c.Y + hs.Y, c.Z), hs), 
				new AABB(new Vector3(c.X + hs.X, c.Y + hs.Y, c.Z), hs),
				
				new AABB(new Vector3(c.X, c.Y, c.Z + hs.Z), hs), 
				new AABB(new Vector3(c.X + hs.X, c.Y, c.Z + hs.Z), hs), 
				new AABB(new Vector3(c.X, c.Y + hs.Y, c.Z + hs.Z), hs), 
				new AABB(new Vector3(c.X + hs.X, c.Y + hs.Y, c.Z + hs.Z), hs)
			}.AsParallel().AsOrdered().Select(BuildSub).AsSequential().Select(x => new Octree(x, maxTrisPerLeaf)).ToList();
			
			A = nodes[0];
			B = nodes[1];
			C = nodes[2];
			D = nodes[3];
			E = nodes[4];
			F = nodes[5];
			G = nodes[6];
			H = nodes[7];
		}
	}
}