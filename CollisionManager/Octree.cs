using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using static System.Console;

namespace CollisionManager {
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
		public readonly Octree[] Nodes;

		public readonly Mesh Leaf;
		public readonly AABB BoundingBox;
		public readonly bool Empty;

		public Octree(Mesh mesh, int maxTrisPerLeaf, AABB? boundingBox = null) {
			BoundingBox = boundingBox ?? mesh.BoundingBox;
			if(mesh.Triangles.Count <= maxTrisPerLeaf) {
				Leaf = mesh;
				if(mesh.Triangles.Count == 0)
					Empty = true;
				//BoundingBox = Leaf.BoundingBox;
				return;
			}

			Mesh BuildSub(AABB bb) {
				var planes = bb.Planes;
				
				var tris = new List<Triangle>();
				foreach(var tri in mesh.Triangles) {
					if(bb.Contains(tri)) {
						tris.Add(tri);
						continue;
					}
					if(!bb.IntersectedBy(tri)) continue;
					
					var inside = tri.AsArray.Where(bb.Contains).ToArray();
					//if(inside.Length != 0) continue;
					//WriteLine($"Triangle {tri} intersects {bb} and {inside.Length} points are inside");
					var splitting = new List<Triangle> { tri };
					foreach(var plane in planes) {
						splitting = splitting.Select(x => x.Split(plane)).SelectMany(x => x).ToList();
						//WriteLine($"Split into {splitting.Count} pieces");
						if(splitting.Count == 0)
							break;
					}
					//foreach(var stri in splitting)
					//	WriteLine($"{(bb.Contains(stri) ? "Inside" : "      ")} {stri}");
					//WriteLine($"Got {splitting.Count} triangles after all the splits");
					splitting = splitting.Where(bb.Contains).ToList();
					//WriteLine($"{splitting.Count} triangles are actually inside");
					if(splitting.Count == 0 && !tri.BoundingBox.Touching(bb))
						Console.WriteLine($"Wtf? {tri} {bb}");
					//if((inside.Length == 1 && splitting.Count != 1) || (inside.Length == 1 && splitting.Count != 1) || splitting.Count == 0)
					//	Environment.Exit(0);
					tris.AddRange(splitting);
				}
				return new Mesh(tris);
			}

			var c = BoundingBox.Min;
			var hs = BoundingBox.Size / 2;
			var nodes = new[] {
				new AABB(c, hs), 
				new AABB(new Vector3(c.X + hs.X, c.Y, c.Z), hs), 
				new AABB(new Vector3(c.X, c.Y + hs.Y, c.Z), hs), 
				new AABB(new Vector3(c.X + hs.X, c.Y + hs.Y, c.Z), hs),
				
				new AABB(new Vector3(c.X, c.Y, c.Z + hs.Z), hs), 
				new AABB(new Vector3(c.X + hs.X, c.Y, c.Z + hs.Z), hs), 
				new AABB(new Vector3(c.X, c.Y + hs.Y, c.Z + hs.Z), hs), 
				new AABB(new Vector3(c.X + hs.X, c.Y + hs.Y, c.Z + hs.Z), hs)
			}/*.AsParallel().AsOrdered()*/.Select(x => (BuildSub(x), x))/*.AsSequential()*/.Select(x => new Octree(x.Item1, maxTrisPerLeaf, x.Item2)).ToArray();
			
			A = nodes[0];
			B = nodes[1];
			C = nodes[2];
			D = nodes[3];
			E = nodes[4];
			F = nodes[5];
			G = nodes[6];
			H = nodes[7];
			
			Nodes = nodes;
			
			BoundingBox = new AABB(Nodes.Select(x => x.BoundingBox).ToList());
		}

		public (Triangle, Vector3)? FindIntersection(Vector3 origin, Vector3 direction) {
			if(!BoundingBox.IntersectedBy(origin, direction)) return null;
			if(Leaf != null) {
				(Triangle, Vector3)? closest = null;
				var distance = float.PositiveInfinity;
				foreach(var triangle in Leaf.Triangles) {
					var hit = triangle.FindIntersection(origin, direction);
					if(hit != null && hit.Value.Item2 < distance) {
						distance = hit.Value.Item2;
						closest = (triangle, hit.Value.Item1);
					}
				}
				return closest;
			}

			(Triangle, Vector3)? closestBox = null;
			var boxDist = float.PositiveInfinity;
			foreach(var node in Nodes) {
				if(node.Empty) continue;
				var ret = node.FindIntersection(origin, direction);
				if(ret != null) {
					var dist = (ret.Value.Item2 - origin).LengthSquared();
					if(dist < boxDist) {
						closestBox = ret;
						boxDist = dist;
					}
				}
			}
			return closestBox;
		}
	}
}