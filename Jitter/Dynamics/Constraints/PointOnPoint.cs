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

using System.Numerics;

#endregion

namespace Jitter.Dynamics.Constraints {
	public class PointOnPoint : Constraint {
		float bias;

		float effectiveMass;

		readonly Vector3[] jacobian = new Vector3[4];
		readonly Vector3 localAnchor1;
		readonly Vector3 localAnchor2;
		Vector3 r1, r2;
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
	    public PointOnPoint(RigidBody body1, RigidBody body2, Vector3 anchor)
			: base(body1, body2) {
			localAnchor1 = (anchor - body1.position).Transform(body1.invOrientation);
			localAnchor2 = (anchor - body2.position).Transform(body2.invOrientation);
		}

		public float AppliedImpulse { get; set; }

	    /// <summary>
	    ///     Defines how big the applied impulses can get.
	    /// </summary>
	    public float Softness { get; set; } = 0.01f;

	    /// <summary>
	    ///     Defines how big the applied impulses can get which correct errors.
	    /// </summary>
	    public float BiasFactor { get; set; } = 0.05f;

	    /// <summary>
	    ///     Called once before iteration starts.
	    /// </summary>
	    /// <param name="timestep">The 5simulation timestep</param>
	    public override void PrepareForIteration(float timestep) {
			r1 = localAnchor1.Transform(ref body1.orientation);
			r2 = localAnchor2.Transform(ref body2.orientation);

			var p1 = body1.position - r1;
			var p2 = body2.position - r2;

			var dp = p2 - p1;

			var deltaLength = dp.Length();

			var n = p2 - p1;
			if(n.LengthSquared() != 0.0f) n.Normalize();

			jacobian[0] = -1.0f * n;
			jacobian[1] = -1.0f * Vector3.Cross(r1, n);
			jacobian[2] = 1.0f * n;
			jacobian[3] = Vector3.Cross(r2, n);

			effectiveMass = body1.inverseMass + body2.inverseMass
			                                  + Vector3.Dot(jacobian[1].Transform(ref body1.invInertiaWorld),
				                                  jacobian[1])
			                                  + Vector3.Dot(jacobian[3].Transform(ref body2.invInertiaWorld),
				                                  jacobian[3]);

			softnessOverDt = Softness / timestep;
			effectiveMass += softnessOverDt;

			effectiveMass = 1.0f / effectiveMass;

			bias = deltaLength * BiasFactor * (1.0f / timestep);

			if(!body1.isStatic) {
				body1.linearVelocity += body1.inverseMass * AppliedImpulse * jacobian[0];
				body1.angularVelocity += (AppliedImpulse * jacobian[1]).Transform(ref body1.invInertiaWorld);
			}

			if(!body2.isStatic) {
				body2.linearVelocity += body2.inverseMass * AppliedImpulse * jacobian[2];
				body2.angularVelocity += (AppliedImpulse * jacobian[3]).Transform(ref body2.invInertiaWorld);
			}
		}

	    /// <summary>
	    ///     Iteratively solve this constraint.
	    /// </summary>
	    public override void Iterate() {
			var jv =
				Vector3.Dot(body1.linearVelocity, jacobian[0]) +
				Vector3.Dot(body1.angularVelocity, jacobian[1]) +
				Vector3.Dot(body2.linearVelocity, jacobian[2]) +
				Vector3.Dot(body2.angularVelocity, jacobian[3]);

			var softnessScalar = AppliedImpulse * softnessOverDt;

			var lambda = -effectiveMass * (jv + bias + softnessScalar);

			AppliedImpulse += lambda;

			if(!body1.isStatic) {
				body1.linearVelocity += body1.inverseMass * lambda * jacobian[0];
				body1.angularVelocity += (lambda * jacobian[1]).Transform(ref body1.invInertiaWorld);
			}

			if(!body2.isStatic) {
				body2.linearVelocity += body2.inverseMass * lambda * jacobian[2];
				body2.angularVelocity += (lambda * jacobian[3]).Transform(ref body2.invInertiaWorld);
			}
		}

		public override void DebugDraw(IDebugDrawer drawer) {
			drawer.DrawLine(body1.position, body1.position + r1);
			drawer.DrawLine(body2.position, body2.position + r2);
		}
	}
}