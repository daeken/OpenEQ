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
using System.Numerics;
using Jitter.LinearMath;

#endregion

namespace Jitter.Dynamics.Constraints.SingleBody {
	#region Constraint Equations

	// Constraint formulation:
	// 
	// C_1 = R1_x - R2_x
	// C_2 = ...
	// C_3 = ...
	//
	// Derivative:
	//
	// dC_1/dt = w1_x - w2_x
	// dC_2/dt = ...
	// dC_3/dt = ...
	//
	// Jacobian:
	// 
	// dC/dt = J*v+b
	//
	// v = (v1x v1y v1z w1x w1y w1z v2x v2y v2z w2x w2y w2z)^(T) 
	//
	//     v1x v1y v1z w1x w1y w1z v2x v2y v2z w2x w2y w2z
	//     -------------------------------------------------
	// J = 0   0   0   1   0    0   0   0   0   -1   0   0   <- dC_1/dt
	//     0   0   0   0   1    0   0   0   0    0  -1   0   <- ...  
	//     0   0   0   0   0    1   0   0   0    0   0  -1   <- ...
	//
	// Effective Mass:
	//
	// 1/m_eff = [J^T * M^-1 * J] = I1^(-1) + I2^(-1)

	#endregion

    /// <summary>
    ///     The body stays at a fixed angle relative to
    ///     world space.
    /// </summary>
    public class FixedAngle : Constraint {
		Vector3 accumulatedImpulse;
		Vector3 bias;

		JMatrix effectiveMass;

		float softnessOverDt;

        /// <summary>
        ///     Constraints two bodies to always have the same relative
        ///     orientation to each other. Combine the AngleConstraint with a PointOnLine
        ///     Constraint to get a prismatic joint.
        /// </summary>
        public FixedAngle(RigidBody body1)
			: base(body1, null) => InitialOrientation = body1.orientation;

        /// <summary>
        ///     Defines how big the applied impulses can get.
        /// </summary>
        public float Softness { get; set; }

        /// <summary>
        ///     Defines how big the applied impulses can get which correct errors.
        /// </summary>
        public float BiasFactor { get; set; } = 0.05f;

		public JMatrix InitialOrientation { get; set; }

        /// <summary>
        ///     Called once before iteration starts.
        /// </summary>
        /// <param name="timestep">The 5simulation timestep</param>
        public override void PrepareForIteration(float timestep) {
			effectiveMass = body1.invInertiaWorld;

			softnessOverDt = Softness / timestep;

			effectiveMass.M11 += softnessOverDt;
			effectiveMass.M22 += softnessOverDt;
			effectiveMass.M33 += softnessOverDt;

			JMatrix.Inverse(ref effectiveMass, out effectiveMass);

			var q = JMatrix.Transpose(InitialOrientation) * body1.orientation;
			Vector3 axis;

			var x = q.M32 - q.M23;
			var y = q.M13 - q.M31;
			var z = q.M21 - q.M12;

			var r = JMath.Sqrt(x * x + y * y + z * z);
			var t = q.M11 + q.M22 + q.M33;

			var angle = MathF.Atan2(r, t - 1);
			axis = new Vector3(x, y, z) * angle;

			if(r != 0.0f) axis = axis * (1.0f / r);

			bias = axis * BiasFactor * (-1.0f / timestep);

			// Apply previous frame solution as initial guess for satisfying the constraint.
			if(!body1.IsStatic) body1.angularVelocity += accumulatedImpulse.Transform(ref body1.invInertiaWorld);
		}

        /// <summary>
        ///     Iteratively solve this constraint.
        /// </summary>
        public override void Iterate() {
			var jv = body1.angularVelocity;

			var softnessVector = accumulatedImpulse * softnessOverDt;

			var lambda = -1.0f * (jv + bias + softnessVector).Transform(ref effectiveMass);

			accumulatedImpulse += lambda;

			if(!body1.IsStatic) body1.angularVelocity += lambda.Transform(ref body1.invInertiaWorld);
		}
	}
}