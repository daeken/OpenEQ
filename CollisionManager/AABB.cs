using System.Collections.Generic;
using System.Linq;
using System.Numerics;

namespace CollisionManager {
	public struct AABB {
		public readonly Vector3 Min, Max;
		public readonly Vector3 Size;
		public readonly Vector3 Center;
		
		public AABB(Vector3 min, Vector3 size) {
			Min = min;
			Max = min + size;
			Size = size;
			Center = min + size / 2;
		}

		public AABB(IReadOnlyList<AABB> set) {
			Min = set.Select(x => x.Min).Aggregate(Vector3.Min);
			Max = set.Select(x => x.Max).Aggregate(Vector3.Max);
			Size = Max - Min;
			Center = Min + Size / 2;
		}

		// TODO: Make this handle cases where a portion of a triangle is within this AABB!
		public bool Contains(Triangle tri) =>
			Contains(tri.A) || Contains(tri.B) || Contains(tri.C) || IntersectedBy(tri);

		static readonly Vector3[] BoxNormals = {
			new Vector3(1, 0, 0), 
			new Vector3(0, 1, 0), 
			new Vector3(0, 0, 1)
		};
		
		public bool IntersectedBy(Triangle tri) {
			var triVerts = tri.AsArray;
			var (triangleMin, triangleMax) = Project(triVerts, new Vector3(1, 0, 0));
			if(triangleMax < Min.X || triangleMin > Max.X) return false;
			(triangleMin, triangleMax) = Project(triVerts, new Vector3(0, 1, 0));
			if(triangleMax < Min.Y || triangleMin > Max.Y) return false;
			(triangleMin, triangleMax) = Project(triVerts, new Vector3(0, 0, 1));
			if(triangleMax < Min.Z || triangleMin > Max.Z) return false;

			var boxVerts = new[] {
				Min, 
				new Vector3(Max.X, Min.Y, Min.Z), 
				new Vector3(Min.X, Max.Y, Min.Z), 
				new Vector3(Max.X, Max.Y, Min.Z), 
				
				new Vector3(Min.X, Min.Y, Max.Z), 
				new Vector3(Max.X, Min.Y, Max.Z), 
				new Vector3(Min.X, Max.Y, Max.Z), 
				new Vector3(Max.X, Max.Y, Max.Z)
			};

			var triangleOffset = Vector3.Dot(tri.Normal, tri.A);
			var (boxMin, boxMax) = Project(boxVerts, tri.Normal);
			if(boxMax < triangleOffset || boxMin > triangleOffset) return false;

			var triangleEdges = new[] {
				tri.A - tri.B, 
				tri.B - tri.C, 
				tri.C - tri.A 
			};
			for(var i = 0; i < 3; ++i)
				for(var j = 0; j < 3; ++j) {
					var axis = Vector3.Cross(triangleEdges[i], BoxNormals[j]);
					(boxMin, boxMax) = Project(boxVerts, axis);
					(triangleMin, triangleMax) = Project(triVerts, axis);
					if(boxMax <= triangleMin || boxMin >= triangleMax) return false;
				}
			
			return true;
		}

		(float Min, float Max) Project(IEnumerable<Vector3> points, Vector3 axis) {
			var min = float.PositiveInfinity;
			var max = float.NegativeInfinity;

			foreach(var p in points) {
				var val = Vector3.Dot(axis, p);
				if(val < min) min = val;
				if(val > max) max = val;
			}

			return (min, max);
		}

		public bool Contains(Vector3 point) =>
			Min.X <= point.X && Min.Y <= point.Y && Min.Z <= point.Z && 
			Max.X >= point.X && Max.Y >= point.Y && Max.Z >= point.Z;

		public bool IntersectedBy(Vector3 origin, Vector3 direction) {
			//if(Contains(origin)) return true;
			
			var tmin = (Min.X - origin.X) / direction.X;
			var tmax = (Max.X - origin.X) / direction.X;
			if(tmin > tmax) { var temp = tmin; tmin = tmax; tmax = temp; }
			
			var tymin = (Min.Y - origin.Y) / direction.Y;
			var tymax = (Max.Y - origin.Y) / direction.Y;
			if(tymin > tymax) { var temp = tymin; tymin = tymax; tymax = temp; }

			if(tmin > tymax || tymin > tmax) return false;

			if(tymin > tmin) tmin = tymin;
			if(tymax < tmax) tmax = tymax;
			
			var tzmin = (Min.Z - origin.Z) / direction.Z;
			var tzmax = (Max.Z - origin.Z) / direction.Z;
			if(tzmin > tzmax) { var temp = tzmin; tzmin = tzmax; tzmax = temp; }

			return tmin <= tzmax && tzmin <= tmax;
		}

		public override string ToString() => $"AABB(Min={Min}, Max={Max})";
	}
}