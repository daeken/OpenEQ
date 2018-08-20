using System;
using System.Numerics;
using Jitter.Dynamics.Constraints;
using Jitter.LinearMath;

namespace Jitter.Dynamics.Joints {
    /// <summary>
    ///     Limited hinge joint.
    /// </summary>
    public class LimitedHingeJoint : Joint {
		readonly PointOnPoint[] worldPointConstraint;


        /// <summary>
        ///     Initializes a new instance of the HingeJoint class.
        /// </summary>
        /// <param name="world">The world class where the constraints get added to.</param>
        /// <param name="body1">The first body connected to the second one.</param>
        /// <param name="body2">The second body connected to the first one.</param>
        /// <param name="position">The position in world space where both bodies get connected.</param>
        /// <param name="hingeAxis">The axis if the hinge.</param>
        public LimitedHingeJoint(World world, RigidBody body1, RigidBody body2, Vector3 position, Vector3 hingeAxis,
			float hingeFwdAngle, float hingeBckAngle)
			: base(world) {
			// Create the hinge first, two point constraints

			worldPointConstraint = new PointOnPoint[2];

			hingeAxis *= 0.5f;

			var pos1 = position + hingeAxis;
			var pos2 = position - hingeAxis;

			worldPointConstraint[0] = new PointOnPoint(body1, body2, pos1);
			worldPointConstraint[1] = new PointOnPoint(body1, body2, pos2);


			// Now the limit, one max distance constraint

			hingeAxis.Normalize();

			// choose a direction that is perpendicular to the hinge
			var perpDir = new Vector3(0, 1, 0);

			if(Vector3.Dot(perpDir, hingeAxis) > 0.1f) perpDir = new Vector3(-1, 0, 0);

			// now make it perpendicular to the hinge
			var sideAxis = Vector3.Cross(hingeAxis, perpDir);
			perpDir = Vector3.Cross(sideAxis, hingeAxis);
			perpDir.Normalize();

			// the length of the "arm" TODO take this as a parameter? what's
			// the effect of changing it?
			var len = 10.0f * 3;

			// Choose a position using that dir. this will be the anchor point
			// for body 0. relative to hinge
			var hingeRelAnchorPos0 = perpDir * len;


			// anchor point for body 2 is chosen to be in the middle of the
			// angle range.  relative to hinge
			var angleToMiddle = 0.5f * (hingeFwdAngle - hingeBckAngle);
			var hingeRelAnchorPos1 =
				hingeRelAnchorPos0.Transform(JMatrix.CreateFromAxisAngle(hingeAxis,
					-angleToMiddle / 360.0f * 2.0f * JMath.Pi));

			// work out the "string" length
			var hingeHalfAngle = 0.5f * (hingeFwdAngle + hingeBckAngle);
			var allowedDistance = len * 2.0f * MathF.Sin(hingeHalfAngle * 0.5f / 360.0f * 2.0f * JMath.Pi);

			var hingePos = body1.Position;
			var relPos0c = hingePos + hingeRelAnchorPos0;
			var relPos1c = hingePos + hingeRelAnchorPos1;

			DistanceConstraint = new PointPointDistance(body1, body2, relPos0c, relPos1c);
			DistanceConstraint.Distance = allowedDistance;
			DistanceConstraint.Behavior = PointPointDistance.DistanceBehavior.LimitMaximumDistance;
		}

		public PointOnPoint PointConstraint1 => worldPointConstraint[0];
		public PointOnPoint PointConstraint2 => worldPointConstraint[1];

		public PointPointDistance DistanceConstraint { get; }


        /// <summary>
        ///     Adds the internal constraints of this joint to the world class.
        /// </summary>
        public override void Activate() {
			World.AddConstraint(worldPointConstraint[0]);
			World.AddConstraint(worldPointConstraint[1]);
			World.AddConstraint(DistanceConstraint);
		}

        /// <summary>
        ///     Removes the internal constraints of this joint from the world class.
        /// </summary>
        public override void Deactivate() {
			World.RemoveConstraint(worldPointConstraint[0]);
			World.RemoveConstraint(worldPointConstraint[1]);
			World.RemoveConstraint(DistanceConstraint);
		}
	}
}