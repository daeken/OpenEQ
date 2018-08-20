/* Copyright (C) <2009-2011> <Thorben Linneweber, Jitter Physics>
* 
*  This software is provided 'as-is', without any express or implied
*  warranty.  In no event will the authors be held liable for any damages
*  arising from the use of this software.
*
*  Permission is granted to anyone to use this software for any purpose,
*  including commercial applications, and to alter it and redistribute it
*  freely, subject to the following restrictions:
*
*  1. The origin of this software must not be misrepresented; you must not
*      claim that you wrote the original software. If you use this software
*      in a product, an acknowledgment in the product documentation would be
*      appreciated but is not required.
*  2. Altered source versions must be plainly marked as such, and must not be
*      misrepresented as being the original software.
*  3. This notice may not be removed or altered from any source distribution. 
*/

#region Using Statements

using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Numerics;
using Jitter.Collision;
using Jitter.Collision.Shapes;
using Jitter.Dynamics.Constraints;
using Jitter.LinearMath;

#endregion

namespace Jitter.Dynamics {
	public class SoftBody : IBroadphaseEntity {
		[Flags]
		public enum SpringType {
			EdgeSpring = 0x02,
			ShearSpring = 0x04,
			BendSpring = 0x08
		}

		bool active = true;

		JBBox box;

		internal DynamicTree<Triangle> dynamicTree = new DynamicTree<Triangle>();
		float mass = 1.0f;

		protected List<MassPoint> points = new List<MassPoint>();


		readonly List<int> queryList = new List<int>();

		readonly SphereShape sphere = new SphereShape(0.1f);

		protected List<Spring> springs = new List<Spring>();

		protected float triangleExpansion = 0.1f;
		protected List<Triangle> triangles = new List<Triangle>();


	    /// <summary>
	    ///     Does create an empty body. Derive from SoftBody and fill
	    ///     EdgeSprings,VertexBodies and Triangles by yourself.
	    /// </summary>
	    public SoftBody() {
		}

	    /// <summary>
	    ///     Creates a 2D-Cloth. Connects Nearest Neighbours (4x, called EdgeSprings) and adds additional
	    ///     shear/bend constraints (4xShear+4xBend).
	    /// </summary>
	    /// <param name="sizeX"></param>
	    /// <param name="sizeY"></param>
	    /// <param name="scale"></param>
	    public SoftBody(int sizeX, int sizeY, float scale) {
			var indices = new List<TriangleVertexIndices>();
			var vertices = new List<Vector3>();

			for(var i = 0; i < sizeY; i++) {
				for(var e = 0; e < sizeX; e++) vertices.Add(new Vector3(i, 0, e) * scale);
			}

			for(var i = 0; i < sizeX - 1; i++) {
				for(var e = 0; e < sizeY - 1; e++) {
					var index = new TriangleVertexIndices();
					{
						index.I0 = (e + 0) * sizeX + i + 0;
						index.I1 = (e + 0) * sizeX + i + 1;
						index.I2 = (e + 1) * sizeX + i + 1;

						indices.Add(index);

						index.I0 = (e + 0) * sizeX + i + 0;
						index.I1 = (e + 1) * sizeX + i + 1;
						index.I2 = (e + 1) * sizeX + i + 0;

						indices.Add(index);
					}
				}
			}

			EdgeSprings = new ReadOnlyCollection<Spring>(springs);
			VertexBodies = new ReadOnlyCollection<MassPoint>(points);
			Triangles = new ReadOnlyCollection<Triangle>(triangles);

			AddPointsAndSprings(indices, vertices);

			for(var i = 0; i < sizeX - 1; i++) {
				for(var e = 0; e < sizeY - 1; e++) {
					var spring = new Spring(points[(e + 0) * sizeX + i + 1], points[(e + 1) * sizeX + i + 0]);
					spring.Softness = 0.01f;
					spring.BiasFactor = 0.1f;
					springs.Add(spring);
				}
			}

			foreach(var spring in springs) {
				var delta = spring.body1.position - spring.body2.position;

				if(delta.Z != 0.0f && delta.X != 0.0f) spring.SpringType = SpringType.ShearSpring;
				else spring.SpringType = SpringType.EdgeSpring;
			}


			for(var i = 0; i < sizeX - 2; i++) {
				for(var e = 0; e < sizeY - 2; e++) {
					var spring1 = new Spring(points[(e + 0) * sizeX + i + 0], points[(e + 0) * sizeX + i + 2]);
					spring1.Softness = 0.01f;
					spring1.BiasFactor = 0.1f;

					var spring2 = new Spring(points[(e + 0) * sizeX + i + 0], points[(e + 2) * sizeX + i + 0]);
					spring2.Softness = 0.01f;
					spring2.BiasFactor = 0.1f;

					spring1.SpringType = SpringType.BendSpring;
					spring2.SpringType = SpringType.BendSpring;

					springs.Add(spring1);
					springs.Add(spring2);
				}
			}
		}

		public SoftBody(List<TriangleVertexIndices> indices, List<Vector3> vertices) {
			EdgeSprings = new ReadOnlyCollection<Spring>(springs);
			VertexBodies = new ReadOnlyCollection<MassPoint>(points);

			AddPointsAndSprings(indices, vertices);
			Triangles = new ReadOnlyCollection<Triangle>(triangles);
		}

		public ReadOnlyCollection<Spring> EdgeSprings { get; }
		public ReadOnlyCollection<MassPoint> VertexBodies { get; }
		public ReadOnlyCollection<Triangle> Triangles { get; }

		public bool SelfCollision { get; set; }

		public float TriangleExpansion {
			get => triangleExpansion;
			set => triangleExpansion = value;
		}

		public float VertexExpansion {
			get => sphere.Radius;
			set => sphere.Radius = value;
		}

		public DynamicTree<Triangle> DynamicTree => dynamicTree;
		public Material Material { get; } = new Material();

		public float Pressure { get; set; }

		public float Mass {
			get => mass;
			set {
				for(var i = 0; i < points.Count; i++) points[i].Mass = value / points.Count;
			}
		}

		public float Volume { get; set; } = 1.0f;

		public int BroadphaseTag { get; set; }

		public object Tag { get; set; }

		public JBBox BoundingBox => box;

		public bool IsStaticOrInactive => !active;

		#region AddPressureForces

		void AddPressureForces(float timeStep) {
			if(Pressure == 0.0f || Volume == 0.0f) return;

			var invVolume = 1.0f / Volume;

			foreach(var t in triangles) {
				var v1 = points[t.indices.I0].position;
				var v2 = points[t.indices.I1].position;
				var v3 = points[t.indices.I2].position;

				var cross = Vector3.Cross(v3 - v1, v2 - v1);
				var center = (v1 + v2 + v3) * (1.0f / 3.0f);

				points[t.indices.I0].AddForce(invVolume * cross * Pressure);
				points[t.indices.I1].AddForce(invVolume * cross * Pressure);
				points[t.indices.I2].AddForce(invVolume * cross * Pressure);
			}
		}

		#endregion

		public void Translate(Vector3 position) {
			foreach(var point in points) point.Position += position;

			Update(float.Epsilon);
		}

		public void AddForce(Vector3 force) {
			// TODO
			throw new NotImplementedException();
		}

		public void Rotate(JMatrix orientation, Vector3 center) {
			for(var i = 0; i < points.Count; i++)
				points[i].position = (points[i].position - center).Transform(ref orientation);
		}

		public Vector3 CalculateCenter() => throw new NotImplementedException();

		HashSet<Edge> GetEdges(List<TriangleVertexIndices> indices) {
			var edges = new HashSet<Edge>();

			for(var i = 0; i < indices.Count; i++) {
				Edge edge;

				edge = new Edge(indices[i].I0, indices[i].I1);
				if(!edges.Contains(edge)) edges.Add(edge);

				edge = new Edge(indices[i].I1, indices[i].I2);
				if(!edges.Contains(edge)) edges.Add(edge);

				edge = new Edge(indices[i].I2, indices[i].I0);
				if(!edges.Contains(edge)) edges.Add(edge);
			}

			return edges;
		}

		public virtual void DoSelfCollision(CollisionDetectedHandler collision) {
			if(!SelfCollision) return;

			Vector3 point, normal;
			float penetration;

			for(var i = 0; i < points.Count; i++) {
				queryList.Clear();
				dynamicTree.Query(queryList, ref points[i].boundingBox);

				for(var e = 0; e < queryList.Count; e++) {
					var t = dynamicTree.GetUserData(queryList[e]);

					if(!(t.VertexBody1 == points[i] || t.VertexBody2 == points[i] || t.VertexBody3 == points[i]))
						if(XenoCollide.Detect(points[i].Shape, t, ref points[i].orientation,
							ref JMatrix.InternalIdentity, points[i].position, Vector3.Zero,
							out point, out normal, out penetration)) {
							var nearest = CollisionSystem.FindNearestTrianglePoint(this, queryList[e], ref point);

							collision(points[i], points[nearest], point, point, normal, penetration);
						}
				}
			}
		}


		void AddPointsAndSprings(List<TriangleVertexIndices> indices, List<Vector3> vertices) {
			for(var i = 0; i < vertices.Count; i++) {
				var point = new MassPoint(sphere, this, Material);
				point.Position = vertices[i];

				point.Mass = 0.1f;

				points.Add(point);
			}

			for(var i = 0; i < indices.Count; i++) {
				var index = indices[i];

				var t = new Triangle(this);

				t.indices = index;
				triangles.Add(t);

				t.boundingBox = JBBox.SmallBox;
				t.boundingBox.AddPoint(points[t.indices.I0].position);
				t.boundingBox.AddPoint(points[t.indices.I1].position);
				t.boundingBox.AddPoint(points[t.indices.I2].position);

				t.dynamicTreeID = dynamicTree.AddProxy(ref t.boundingBox, t);
			}

			var edges = GetEdges(indices);

			var count = 0;

			foreach(var edge in edges) {
				var spring = new Spring(points[edge.Index1], points[edge.Index2]);
				spring.Softness = 0.01f;
				spring.BiasFactor = 0.1f;
				spring.SpringType = SpringType.EdgeSpring;

				springs.Add(spring);
				count++;
			}
		}

		public void SetSpringValues(float bias, float softness) {
			SetSpringValues(SpringType.EdgeSpring | SpringType.ShearSpring | SpringType.BendSpring,
				bias, softness);
		}

		public void SetSpringValues(SpringType type, float bias, float softness) {
			for(var i = 0; i < springs.Count; i++)
				if((springs[i].SpringType & type) != 0) {
					springs[i].Softness = softness;
					springs[i].BiasFactor = bias;
				}
		}

		public virtual void Update(float timestep) {
			active = false;

			foreach(var point in points)
				if(point.isActive && !point.isStatic) {
					active = true;
					break;
				}

			if(!active) return;

			box = JBBox.SmallBox;
			Volume = 0.0f;
			mass = 0.0f;

			foreach(var point in points) {
				mass += point.Mass;
				box.AddPoint(point.position);
			}

			box.Min -= new Vector3(TriangleExpansion);
			box.Max += new Vector3(TriangleExpansion);

			foreach(var t in triangles) {
				// Update bounding box and move proxy in dynamic tree.
				var prevCenter = t.boundingBox.Center;
				t.UpdateBoundingBox();

				var linVel = t.VertexBody1.linearVelocity +
				             t.VertexBody2.linearVelocity +
				             t.VertexBody3.linearVelocity;

				linVel *= 1.0f / 3.0f;

				dynamicTree.MoveProxy(t.dynamicTreeID, ref t.boundingBox, linVel * timestep);

				var v1 = points[t.indices.I0].position;
				var v2 = points[t.indices.I1].position;
				var v3 = points[t.indices.I2].position;

				Volume -= ((v2.Y - v1.Y) * (v3.Z - v1.Z) -
				           (v2.Z - v1.Z) * (v3.Y - v1.Y)) * (v1.X + v2.X + v3.X);
			}

			Volume /= 6.0f;

			AddPressureForces(timestep);
		}

		#region public class Spring : Constraint

		public class Spring : Constraint {
			public enum DistanceBehavior {
				LimitDistance,
				LimitMaximumDistance,
				LimitMinimumDistance
			}

			float bias;

			float effectiveMass;

			readonly Vector3[] jacobian = new Vector3[2];

			bool skipConstraint;
			float softnessOverDt;

		    /// <summary>
		    ///     Initializes a new instance of the DistanceConstraint class.
		    /// </summary>
		    /// <param name="body1">The first body.</param>
		    /// <param name="body2">The second body.</param>
		    /// <param name="anchor1">
		    ///     The anchor point of the first body in world space.
		    ///     The distance is given by the initial distance between both anchor points.
		    /// </param>
		    /// <param name="anchor2">
		    ///     The anchor point of the second body in world space.
		    ///     The distance is given by the initial distance between both anchor points.
		    /// </param>
		    public Spring(RigidBody body1, RigidBody body2)
				: base(body1, body2) => Distance = (body1.position - body2.position).Length();

			public SpringType SpringType { get; set; }

			public float AppliedImpulse { get; set; }

		    /// <summary>
		    /// </summary>
		    public float Distance { get; set; }

		    /// <summary>
		    /// </summary>
		    public DistanceBehavior Behavior { get; set; } = DistanceBehavior.LimitDistance;

		    /// <summary>
		    ///     Defines how big the applied impulses can get.
		    /// </summary>
		    public float Softness { get; set; } = 0.01f;

		    /// <summary>
		    ///     Defines how big the applied impulses can get which correct errors.
		    /// </summary>
		    public float BiasFactor { get; set; } = 0.1f;

		    /// <summary>
		    ///     Called once before iteration starts.
		    /// </summary>
		    /// <param name="timestep">The 5simulation timestep</param>
		    public override void PrepareForIteration(float timestep) {
				Vector3 dp;
				dp = body2.position - body1.position;

				var deltaLength = dp.Length() - Distance;

				if(Behavior == DistanceBehavior.LimitMaximumDistance && deltaLength <= 0.0f)
					skipConstraint = true;
				else if(Behavior == DistanceBehavior.LimitMinimumDistance && deltaLength >= 0.0f)
					skipConstraint = true;
				else {
					skipConstraint = false;

					var n = dp;
					if(n.LengthSquared() != 0.0f) n.Normalize();

					jacobian[0] = -1.0f * n;
					//jacobian[1] = -1.0f * (r1 % n);
					jacobian[1] = 1.0f * n;
					//jacobian[3] = (r2 % n);

					effectiveMass = body1.inverseMass + body2.inverseMass;

					softnessOverDt = Softness / timestep;
					effectiveMass += softnessOverDt;

					effectiveMass = 1.0f / effectiveMass;

					bias = deltaLength * BiasFactor * (1.0f / timestep);

					if(!body1.isStatic) body1.linearVelocity += body1.inverseMass * AppliedImpulse * jacobian[0];

					if(!body2.isStatic) body2.linearVelocity += body2.inverseMass * AppliedImpulse * jacobian[1];
				}
			}

		    /// <summary>
		    ///     Iteratively solve this constraint.
		    /// </summary>
		    public override void Iterate() {
				if(skipConstraint) return;

				var jv = Vector3.Dot(body1.linearVelocity, jacobian[0]);
				jv += Vector3.Dot(body2.linearVelocity, jacobian[1]);

				var softnessScalar = AppliedImpulse * softnessOverDt;

				var lambda = -effectiveMass * (jv + bias + softnessScalar);

				if(Behavior == DistanceBehavior.LimitMinimumDistance) {
					var previousAccumulatedImpulse = AppliedImpulse;
					AppliedImpulse = JMath.Max(AppliedImpulse + lambda, 0);
					lambda = AppliedImpulse - previousAccumulatedImpulse;
				} else if(Behavior == DistanceBehavior.LimitMaximumDistance) {
					var previousAccumulatedImpulse = AppliedImpulse;
					AppliedImpulse = JMath.Min(AppliedImpulse + lambda, 0);
					lambda = AppliedImpulse - previousAccumulatedImpulse;
				} else
					AppliedImpulse += lambda;

				Vector3 temp;

				if(!body1.isStatic) {
					temp = jacobian[0] * (lambda * body1.inverseMass);
					body1.linearVelocity = temp + body1.linearVelocity;
				}

				if(!body2.isStatic) {
					temp = jacobian[1] * (lambda * body2.inverseMass);
					body2.linearVelocity = temp + body2.linearVelocity;
				}
			}

			public override void DebugDraw(IDebugDrawer drawer) {
				drawer.DrawLine(body1.position, body2.position);
			}
		}

		#endregion

		#region public class MassPoint : RigidBody

		public class MassPoint : RigidBody {
			public MassPoint(Shape shape, SoftBody owner, Material material)
				: base(shape, material, true) => SoftBody = owner;

			public SoftBody SoftBody { get; }
		}

		#endregion

		#region public class Triangle : ISupportMappable

		public class Triangle : ISupportMappable {
			internal JBBox boundingBox;
			internal int dynamicTreeID;
			internal TriangleVertexIndices indices;

			public Triangle(SoftBody owner) => Owner = owner;

			public SoftBody Owner { get; }


			public JBBox BoundingBox => boundingBox;
			public int DynamicTreeID => dynamicTreeID;

			public TriangleVertexIndices Indices => indices;

			public MassPoint VertexBody1 => Owner.points[indices.I0];
			public MassPoint VertexBody2 => Owner.points[indices.I1];
			public MassPoint VertexBody3 => Owner.points[indices.I2];

			public void SupportMapping(ref Vector3 direction, out Vector3 result) {
				var min = Vector3.Dot(Owner.points[indices.I0].position, direction);
				var dot = Vector3.Dot(Owner.points[indices.I1].position, direction);

				var minVertex = Owner.points[indices.I0].position;

				if(dot > min) {
					min = dot;
					minVertex = Owner.points[indices.I1].position;
				}

				dot = Vector3.Dot(Owner.points[indices.I2].position, direction);
				if(dot > min) {
					min = dot;
					minVertex = Owner.points[indices.I2].position;
				}


				Vector3 exp;
				exp = Vector3.Normalize(direction);
				exp *= Owner.triangleExpansion;
				result = minVertex + exp;
			}

			public void SupportCenter(out Vector3 center) {
				center = Owner.points[indices.I0].position;
				center = center + Owner.points[indices.I1].position;
				center = center + Owner.points[indices.I2].position;
				center = center * (1.0f / 3.0f);
			}

			public void GetNormal(out Vector3 normal) {
				Vector3 sum;
				sum = Owner.points[indices.I1].position - Owner.points[indices.I0].position;
				normal = Owner.points[indices.I2].position - Owner.points[indices.I0].position;
				normal = Vector3.Cross(sum, normal);
			}

			public void UpdateBoundingBox() {
				boundingBox = JBBox.SmallBox;
				boundingBox.AddPoint(ref Owner.points[indices.I0].position);
				boundingBox.AddPoint(ref Owner.points[indices.I1].position);
				boundingBox.AddPoint(ref Owner.points[indices.I2].position);

				boundingBox.Min -= new Vector3(Owner.triangleExpansion);
				boundingBox.Max += new Vector3(Owner.triangleExpansion);
			}

			public float CalculateArea() => Vector3.Cross(
				Owner.points[indices.I1].position - Owner.points[indices.I0].position,
				Owner.points[indices.I2].position - Owner.points[indices.I0].position).Length();
		}

		#endregion

		struct Edge {
			public readonly int Index1;
			public readonly int Index2;

			public Edge(int index1, int index2) {
				Index1 = index1;
				Index2 = index2;
			}

			public override int GetHashCode() => Index1.GetHashCode() + Index2.GetHashCode();

			public override bool Equals(object obj) {
				var e = (Edge) obj;
				return e.Index1 == Index1 && e.Index2 == Index2 || e.Index1 == Index2 && e.Index2 == Index1;
			}
		}
	}
}