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

namespace Jitter.Dynamics.Constraints.SingleBody {
	public class PointOnPoint : Constraint {
		float bias;

		float effectiveMass;

		readonly Vector3[] jacobian = new Vector3[2];
		readonly Vector3 localAnchor1;

		Vector3 r1;
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
	    public PointOnPoint(RigidBody body, Vector3 localAnchor)
			: base(body, null) {
			localAnchor1 = localAnchor;

			Anchor = body.position + localAnchor.Transform(ref body.orientation);
		}

		public float AppliedImpulse { get; set; }

	    /// <summary>
	    ///     Defines how big the applied impulses can get.
	    /// </summary>
	    public float Softness { get; set; } = 0.01f;

	    /// <summary>
	    ///     The anchor point in the world.
	    /// </summary>
	    public Vector3 Anchor { get; set; }


	    /// <summary>
	    ///     Defines how big the applied impulses can get which correct errors.
	    /// </summary>
	    public float BiasFactor { get; set; } = 0.1f;

	    /// <summary>
	    ///     Called once before iteration starts.
	    /// </summary>
	    /// <param name="timestep">The 5simulation timestep</param>
	    public override void PrepareForIteration(float timestep) {
			Vector3 p1, dp;
			localAnchor1.Transform(ref body1.orientation, out r1);
			p1 = body1.position + r1;

			dp = p1 - Anchor;
			var deltaLength = dp.Length();

			var n = Anchor - p1;
			if(n.LengthSquared() != 0.0f) n.Normalize();

			jacobian[0] = -1.0f * n;
			jacobian[1] = -1.0f * Vector3.Cross(r1, n);

			effectiveMass = body1.inverseMass +
			                Vector3.Dot(jacobian[1].Transform(ref body1.invInertiaWorld), jacobian[1]);

			softnessOverDt = Softness / timestep;
			effectiveMass += softnessOverDt;

			effectiveMass = 1.0f / effectiveMass;

			bias = deltaLength * BiasFactor * (1.0f / timestep);

			if(!body1.isStatic) {
				body1.linearVelocity += body1.inverseMass * AppliedImpulse * jacobian[0];
				body1.angularVelocity += (AppliedImpulse * jacobian[1]).Transform(ref body1.invInertiaWorld);
			}
		}

	    /// <summary>
	    ///     Iteratively solve this constraint.
	    /// </summary>
	    public override void Iterate() {
			var jv =
				Vector3.Dot(body1.linearVelocity, jacobian[0]) +
				Vector3.Dot(body1.angularVelocity, jacobian[1]);

			var softnessScalar = AppliedImpulse * softnessOverDt;

			var lambda = -effectiveMass * (jv + bias + softnessScalar);

			AppliedImpulse += lambda;

			if(!body1.isStatic) {
				body1.linearVelocity += body1.inverseMass * lambda * jacobian[0];
				body1.angularVelocity += (lambda * jacobian[1]).Transform(ref body1.invInertiaWorld);
			}
		}

		public override void DebugDraw(IDebugDrawer drawer) {
			drawer.DrawPoint(Anchor);
		}
	}
}