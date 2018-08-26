using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using System.Threading.Tasks;
using static System.Console;

namespace CollisionManager {
	public class Octree {
		public readonly Octree[] Nodes;
		public readonly Mesh Leaf;
		public readonly AABB BoundingBox;
		public readonly bool Empty;

		public Octree(Mesh mesh, int maxTrisPerLeaf) {
			BoundingBox = mesh.BoundingBox;
			if(mesh.Triangles.Count <= maxTrisPerLeaf) {
				Leaf = mesh;
				if(mesh.Triangles.Count == 0)
					Empty = true;
				return;
			}

			var planes = BoundingBox.MidPlanes;
			var triLists = Enumerable.Range(0, 8).Select(x => new ConcurrentBag<Triangle>()).ToArray();

			void Divide(Triangle tri, int nodeIdx, int planeIdx) {
				if(planeIdx == 3) {
					triLists[nodeIdx].Add(tri);
					return;
				}

				var plane = planes[planeIdx];
				var sideA = Vector3.Dot(tri.A, plane.Normal) - plane.Distance >= 0;
				var sideB = Vector3.Dot(tri.B, plane.Normal) - plane.Distance >= 0;
				var sideC = Vector3.Dot(tri.C, plane.Normal) - plane.Distance >= 0;

				if(sideA == sideB && sideB == sideC)
					Divide(tri, nodeIdx | ((sideA ? 1 : 0) << planeIdx), planeIdx + 1);
				else
					foreach(var sub in tri.Split(plane)) {
						var side = Vector3.Dot(sub.A, plane.Normal) - plane.Distance;
						if(side >= 0)
							Divide(sub, nodeIdx | (1 << planeIdx), planeIdx + 1);
						else
							Divide(sub, nodeIdx, planeIdx + 1);
					}
			}
			
			Parallel.ForEach(mesh.Triangles, tri => Divide(tri, 0, 0));
			
			Nodes = triLists.Select(x => new Octree(new Mesh(x), maxTrisPerLeaf)).ToArray();
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