using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using System.Threading.Tasks;
using static System.Console;

namespace CollisionManager {
	public class Octree {
		readonly Octree[] Nodes;
		readonly Mesh Leaf;
		readonly AABB BoundingBox;
		readonly bool Empty;

		public Octree(Mesh mesh, int maxTrisPerLeaf, AABB? boundingBox = null) {
			BoundingBox = boundingBox ?? mesh.BoundingBox;
			if(mesh.Triangles.Count <= maxTrisPerLeaf) {
				Leaf = mesh;
				if(mesh.Triangles.Count == 0)
					Empty = true;
				BoundingBox = Leaf.BoundingBox;
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
						var side = Vector3.Dot(sub.Center, plane.Normal) - plane.Distance >= 0;
						Divide(sub, nodeIdx | ((side ? 1 : 0) << planeIdx), planeIdx + 1);
					}
			}
			
			Parallel.ForEach(mesh.Triangles, tri => Divide(tri, 0, 0));

			var mm = (BoundingBox.Min, BoundingBox.Center);
			var ms = BoundingBox.Size / 2;
			var boundingMins = new[] {
				mm.Select(0, 0, 0), 
				mm.Select(0, 0, 1), 
				mm.Select(0, 1, 0), 
				mm.Select(0, 1, 1), 
				mm.Select(1, 0, 0), 
				mm.Select(1, 0, 1), 
				mm.Select(1, 1, 0), 
				mm.Select(1, 1, 1)
			};
			
			Nodes = triLists.Select((x, i) => new Octree(new Mesh(x), maxTrisPerLeaf, new AABB(boundingMins[i], ms))).ToArray();
		}

		public (Triangle, Vector3)? FindIntersectionSlow(Vector3 origin, Vector3 direction) {
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
				var ret = node.FindIntersectionSlow(origin, direction);
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

		public (Triangle, Vector3)? FindIntersection(Vector3 _origin, Vector3 _direction) {
			// https://github.com/dotnet/coreclr/issues/19674
			var origin = _origin;
			var direction = _direction;
			
			var a = 0;

			if(direction.X < 0) {
				origin.X = BoundingBox.Min.X + BoundingBox.Max.X - origin.X;
				direction.X = -direction.X;
				a |= 4;
			}
			if(direction.Y < 0) {
				origin.Y = BoundingBox.Min.Y + BoundingBox.Max.Y - origin.Y;
				direction.Y = -direction.Y;
				a |= 2;
			}
			if(direction.Z < 0) {
				origin.Z = BoundingBox.Min.Z + BoundingBox.Max.Z - origin.Z;
				direction.Z = -direction.Z;
				a |= 1;
			}

			var t0 = (BoundingBox.Min - origin) / direction;
			var t1 = (BoundingBox.Max - origin) / direction;
			if(t0.ComponentMax() < t1.ComponentMin())
				return SubIntersection(t0, t1, _origin, _direction, a);

			return null;
		}

		static int FirstNode(Vector3 t0, Vector3 tm) {
			var tmax = t0.ComponentMax();
			if(t0.X == tmax) return (tm.Y < t0.X ? 2 : 0) | (tm.Z < t0.X ? 1 : 0);
			if(t0.Y == tmax) return (tm.X < t0.Y ? 4 : 0) | (tm.Z < t0.Y ? 1 : 0);
			return (tm.X < t0.Z ? 4 : 0) | (tm.Y < t0.Z ? 2 : 0);
		}

		(Triangle, Vector3)? SubIntersection(Vector3 t0, Vector3 t1, Vector3 origin, Vector3 direction, int a) {
			if(t1.X < 0 || t1.Y < 0 || t1.Z < 0) return null;
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

			var tm = (t0 + t1) * 0.5f;
			var curNode = FirstNode(t0, tm);
			while(curNode < 8) {
				var child = Nodes[a ^ curNode];
				Vector3 f0, f1;

				int NewNode(int x, int y, int z) {
					if(f1.X < f1.Y) {
						if(f1.X < f1.Z) return x;
					} else if(f1.Y < f1.Z) return y;
					return z;
				}

				switch(curNode) {
					case 0:
						f0 = t0;
						f1 = tm;
						curNode = NewNode(4, 2, 1);
						break;
					case 1:
						f0 = new Vector3(t0.X, t0.Y, tm.Z);
						f1 = new Vector3(tm.X, tm.Y, t1.Z);
						curNode = NewNode(5, 3, 8);
						break;
					case 2:
						f0 = new Vector3(t0.X, tm.Y, t0.Z);
						f1 = new Vector3(tm.X, t1.Y, tm.Z);
						curNode = NewNode(6, 8, 3);
						break;
					case 3:
						f0 = new Vector3(t0.X, tm.Y, tm.Z);
						f1 = new Vector3(tm.X, t1.Y, t1.Z);
						curNode = NewNode(7, 8, 8);
						break;
					case 4:
						f0 = new Vector3(tm.X, t0.Y, t0.Z);
						f1 = new Vector3(t1.X, tm.Y, tm.Z);
						curNode = NewNode(8, 6, 5);
						break;
					case 5:
						f0 = new Vector3(tm.X, t0.Y, tm.Z);
						f1 = new Vector3(t1.X, tm.Y, t1.Z);
						curNode = NewNode(8, 7, 8);
						break;
					case 6:
						f0 = new Vector3(tm.X, tm.Y, t0.Z);
						f1 = new Vector3(t1.X, t1.Y, tm.Z);
						curNode = NewNode(8, 8, 7);
						break;
					default: // case 7
						f0 = tm;
						f1 = t1;
						curNode = 8;
						break;
				}

				var ret = child.SubIntersection(f0, f1, origin, direction, a);
				if(ret != null) return ret;
			}
			return null;
		}
	}
}