using System;
using System.Collections.Concurrent;
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
		readonly float Diameter;

		public Octree(Mesh mesh, int maxTrisPerLeaf, AABB? boundingBox = null, int depth = 0) {
			BoundingBox = boundingBox/*?.Union(mesh.BoundingBox)*/ ?? mesh.BoundingBox;
			if(BoundingBox.Size == Vector3.Zero) { Empty = true; return; }
			Diameter = BoundingBox.Size.ComponentMax();
			if(mesh.Triangles.Count <= maxTrisPerLeaf) {
				Leaf = mesh;
				if(mesh.Triangles.Count == 0)
					Empty = true;
				BoundingBox = Leaf.BoundingBox;
				Diameter = BoundingBox.Size.ComponentMax();
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
				var sideA = Vector3.Dot(tri.A, plane.Normal) - plane.Distance >= -0.0001f;
				var sideB = Vector3.Dot(tri.B, plane.Normal) - plane.Distance >= -0.0001f;
				var sideC = Vector3.Dot(tri.C, plane.Normal) - plane.Distance >= -0.0001f;

				if(sideA == sideB && sideB == sideC)
					Divide(tri, nodeIdx | ((sideA ? 1 : 0) << planeIdx), planeIdx + 1);
				else {
					Divide(tri, nodeIdx, planeIdx + 1);
					Divide(tri, nodeIdx | (1 << planeIdx), planeIdx + 1);
				}
			}
			
			Parallel.ForEach(mesh.Triangles, tri => Divide(tri, 0, 0));
			
			var fc = triLists.First(x => x.Count != 0).Count;
			if(triLists.All(x => x.Count == fc)) {
				WriteLine($"Making premature leaf with {fc} triangles instead of {maxTrisPerLeaf}");
				Leaf = mesh;
				BoundingBox = Leaf.BoundingBox;
				return;
			}
			
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
			
			Nodes = triLists.Select((x, i) => new Octree(new Mesh(x), maxTrisPerLeaf, new AABB(boundingMins[i], ms), depth + 1)).ToArray();
		}

		public (Triangle, Vector3)? FindIntersectionSlow(Vector3 origin, Vector3 direction) {
			if(Empty || !BoundingBox.IntersectedBy(origin, direction)) return null;
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

		public (Triangle, Vector3)? FindIntersectionCustom(Vector3 origin, Vector3 direction, float? spread = null) {
			Vector3? perp1 = null, perp2 = null;
			if(spread != null) {
				perp1 = Vector3.Normalize(Vector3.Cross(direction.Y == 0 && direction.Z == 0 ? Vector3.UnitY : Vector3.UnitX, direction)) * spread.Value;
				perp2 = Vector3.Normalize(Vector3.Cross(perp1.Value, direction)) * spread.Value;
			}
			return !BoundingBox.Contains(origin)
				? FindIntersectionSlow(origin, direction)
				: SubIntersectionCustom(origin, direction, perp1, perp2);
		}

		(Triangle, Vector3)? SubIntersectionCustom(Vector3 _origin, Vector3 direction, Vector3? spread1, Vector3? spread2) {
			if(Empty) return null;
			var origin = _origin;
			if(Leaf != null) {
				(Triangle, Vector3)? closest = null;
				var distance = float.PositiveInfinity;
				foreach(var triangle in Leaf.Triangles) {
					var hit = triangle.FindIntersection(origin, direction);
					if(hit == null && spread1 != null && spread2 != null) {
						hit = triangle.FindIntersection(origin + spread1.Value, direction);
						if(hit == null) hit = triangle.FindIntersection(origin - spread1.Value, direction);
						if(hit == null) hit = triangle.FindIntersection(origin + spread2.Value, direction);
						if(hit == null) hit = triangle.FindIntersection(origin - spread2.Value, direction);
					}

					if(hit != null && hit.Value.Item2 < distance) {
						distance = hit.Value.Item2;
						closest = (triangle, hit.Value.Item1);
					}
				}
				return closest;
			}
			
			var planes = BoundingBox.MidPlanes;
			var side = (
				X: Vector3.Dot(origin, planes[2].Normal) - planes[2].Distance >= 0,
				Y: Vector3.Dot(origin, planes[1].Normal) - planes[1].Distance >= 0,
				Z: Vector3.Dot(origin, planes[0].Normal) - planes[0].Distance >= 0
			);
			var xDist = side.X == direction.X < 0
				? planes[2].RayDistance(origin, direction)
				: float.PositiveInfinity;
			var yDist = side.Y == direction.Y < 0
				? planes[1].RayDistance(origin, direction)
				: float.PositiveInfinity;
			var zDist = side.Z == direction.Z < 0
				? planes[0].RayDistance(origin, direction)
				: float.PositiveInfinity;
			for(var i = 0; i < 3; ++i) {
				var idx = (side.Z ? 1 : 0) | (side.Y ? 2 : 0) | (side.X ? 4 : 0);
				var ret = Nodes[idx].SubIntersectionCustom(origin, direction, spread1, spread2);
				if(ret != null) return ret;

				var minDist = MathF.Min(MathF.Min(xDist, yDist), zDist);
				if(float.IsInfinity(minDist) || minDist > Diameter) return null;
				
				origin = _origin + direction * minDist;
				if(!BoundingBox.Contains(origin)) return null;
				if(minDist == xDist) { side.X = !side.X; xDist = float.PositiveInfinity; }
				else if(minDist == yDist) { side.Y = !side.Y; yDist = float.PositiveInfinity; }
				else if(minDist == zDist) { side.Z = !side.Z; zDist = float.PositiveInfinity; }
			}

			return null;
		}

		public (Triangle, Vector3)? FindIntersection(Vector3 _origin, Vector3 _direction) {
			// TODO: https://github.com/dotnet/coreclr/issues/19674
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