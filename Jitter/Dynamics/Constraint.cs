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
using System.Threading;

#endregion

namespace Jitter.Dynamics.Constraints {
	public interface IConstraint {
		RigidBody Body1 { get; }

	    /// <summary>
	    ///     Gets the second body. Can be null.
	    /// </summary>
	    RigidBody Body2 { get; }

		void PrepareForIteration(float timestep);
		void Iterate();
	}

    /// <summary>
    ///     A constraints forces a body to behave in a specific way.
    /// </summary>
    public abstract class Constraint : IConstraint, IDebugDrawable, IComparable<Constraint> {
		static int instanceCount;
		internal RigidBody body1;
		internal RigidBody body2;
		readonly int instance;

        /// <summary>
        ///     Constructor.
        /// </summary>
        /// <param name="body1">The first body which should get constrained. Can be null.</param>
        /// <param name="body2">The second body which should get constrained. Can be null.</param>
        public Constraint(RigidBody body1, RigidBody body2) {
			this.body1 = body1;
			this.body2 = body2;

			instance = Interlocked.Increment(ref instanceCount);

			// calling body.update does not hurt
			// if the user set orientations all
			// inverse orientations etc. get also
			// recalculated.
			if(body1 != null) body1.Update();
			if(body2 != null) body2.Update();
		}


		public int CompareTo(Constraint other) {
			if(other.instance < instance) return -1;
			if(other.instance > instance) return 1;
			return 0;
		}

        /// <summary>
        ///     Gets the first body. Can be null.
        /// </summary>
        public RigidBody Body1 => body1;

        /// <summary>
        ///     Gets the second body. Can be null.
        /// </summary>
        public RigidBody Body2 => body2;

        /// <summary>
        ///     Called once before iteration starts.
        /// </summary>
        /// <param name="timestep">The simulation timestep</param>
        public abstract void PrepareForIteration(float timestep);

        /// <summary>
        ///     Iteratively solve this constraint.
        /// </summary>
        public abstract void Iterate();

		public virtual void DebugDraw(IDebugDrawer drawer) {
			//throw new NotImplementedException();
		}
	}
}