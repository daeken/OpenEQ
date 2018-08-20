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
using Jitter.Dynamics.Constraints;

#endregion

namespace Jitter.Dynamics.Joints {
	public class PrismaticJoint : Joint {
		// form prismatic joint

		public PrismaticJoint(World world, RigidBody body1, RigidBody body2)
			: base(world) {
			FixedAngleConstraint = new FixedAngle(body1, body2);
			PointOnLineConstraint = new PointOnLine(body1, body2, body1.position, body2.position);
		}

		public PrismaticJoint(World world, RigidBody body1, RigidBody body2, float minimumDistance,
			float maximumDistance)
			: base(world) {
			FixedAngleConstraint = new FixedAngle(body1, body2);
			PointOnLineConstraint = new PointOnLine(body1, body2, body1.position, body2.position);

			MinimumDistanceConstraint = new PointPointDistance(body1, body2, body1.position, body2.position);
			MinimumDistanceConstraint.Behavior = PointPointDistance.DistanceBehavior.LimitMinimumDistance;
			MinimumDistanceConstraint.Distance = minimumDistance;

			MaximumDistanceConstraint = new PointPointDistance(body1, body2, body1.position, body2.position);
			MaximumDistanceConstraint.Behavior = PointPointDistance.DistanceBehavior.LimitMaximumDistance;
			MaximumDistanceConstraint.Distance = maximumDistance;
		}


		public PrismaticJoint(World world, RigidBody body1, RigidBody body2, Vector3 pointOnBody1, Vector3 pointOnBody2)
			: base(world) {
			FixedAngleConstraint = new FixedAngle(body1, body2);
			PointOnLineConstraint = new PointOnLine(body1, body2, pointOnBody1, pointOnBody2);
		}


		public PrismaticJoint(World world, RigidBody body1, RigidBody body2, Vector3 pointOnBody1, Vector3 pointOnBody2,
			float maximumDistance, float minimumDistance)
			: base(world) {
			FixedAngleConstraint = new FixedAngle(body1, body2);
			PointOnLineConstraint = new PointOnLine(body1, body2, pointOnBody1, pointOnBody2);
		}

		public PointPointDistance MaximumDistanceConstraint { get; }

		public PointPointDistance MinimumDistanceConstraint { get; }

		public FixedAngle FixedAngleConstraint { get; }

		public PointOnLine PointOnLineConstraint { get; }

		public override void Activate() {
			if(MaximumDistanceConstraint != null) World.AddConstraint(MaximumDistanceConstraint);
			if(MinimumDistanceConstraint != null) World.AddConstraint(MinimumDistanceConstraint);

			World.AddConstraint(FixedAngleConstraint);
			World.AddConstraint(PointOnLineConstraint);
		}

		public override void Deactivate() {
			if(MaximumDistanceConstraint != null) World.RemoveConstraint(MaximumDistanceConstraint);
			if(MinimumDistanceConstraint != null) World.RemoveConstraint(MinimumDistanceConstraint);

			World.RemoveConstraint(FixedAngleConstraint);
			World.RemoveConstraint(PointOnLineConstraint);
		}
	}
}