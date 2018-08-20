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
using System.Numerics;
using System.Threading;
using Jitter.Collision;
using Jitter.Collision.Shapes;
using Jitter.DataStructures;
using Jitter.Dynamics.Constraints;
using Jitter.LinearMath;

#endregion

namespace Jitter.Dynamics {
	public enum RigidBodyIndex {
		RigidBody1,
		RigidBody2
	}

    /// <summary>
    ///     The RigidBody class.
    /// </summary>
    public class RigidBody : IBroadphaseEntity, IDebugDrawable, IEquatable<RigidBody>, IComparable<RigidBody> {
		[Flags]
		public enum DampingType {
			None = 0x00,
			Angular = 0x01,
			Linear = 0x02
		}


		static int instanceCount;
		internal bool affectedByGravity = true;
		internal Vector3 angularVelocity;

		internal HashSet<Arbiter> arbiters = new HashSet<Arbiter>();

		internal JBBox boundingBox;

		internal List<RigidBody> connections = new List<RigidBody>();
		internal HashSet<Constraint> constraints = new HashSet<Constraint>();

		bool enableDebugDraw;

		internal Vector3 force, torque;

		readonly int hashCode;

		List<Vector3> hullPoints = new List<Vector3>();

		internal float inactiveTime;

		internal JMatrix inertia;
		readonly int instance;

		internal int internalIndex = 0;
		internal float inverseMass;
		internal JMatrix invInertia;

		internal JMatrix invInertiaWorld;
		internal JMatrix invOrientation;

		internal bool isActive = true;

		internal CollisionIsland island;

		internal bool isParticle;
		internal bool isStatic;
		internal Vector3 linearVelocity;

		internal int marker = 0;

		internal Material material;
		internal JMatrix orientation;
		internal Vector3 position;

		Shape shape;


		internal Vector3 sweptDirection = Vector3.Zero;

		ShapeUpdatedHandler updatedHandler;

		protected bool useShapeMassProperties = true;


		public RigidBody(Shape shape)
			: this(shape, new Material(), false) {
		}

        /// <summary>
        ///     Initializes a new instance of the RigidBody class.
        /// </summary>
        /// <param name="shape">The shape of the body.</param>
        public RigidBody(Shape shape, Material material)
			: this(shape, material, false) {
		}

        /// <summary>
        ///     Initializes a new instance of the RigidBody class.
        /// </summary>
        /// <param name="shape">The shape of the body.</param>
        /// <param name="isParticle">
        ///     If set to true the body doesn't rotate.
        ///     Also contacts are only solved for the linear motion part.
        /// </param>
        public RigidBody(Shape shape, Material material, bool isParticle) {
			Arbiters = new ReadOnlyHashset<Arbiter>(arbiters);
			Constraints = new ReadOnlyHashset<Constraint>(constraints);

			instance = Interlocked.Increment(ref instanceCount);
			hashCode = CalculateHash(instance);

			Shape = shape;
			orientation = JMatrix.Identity;

			if(!isParticle) {
				updatedHandler = ShapeUpdated;
				Shape.ShapeUpdated += updatedHandler;
				SetMassProperties();
			} else {
				inertia = JMatrix.Zero;
				invInertia = invInertiaWorld = JMatrix.Zero;
				invOrientation = orientation = JMatrix.Identity;
				inverseMass = 1.0f;
			}

			this.material = material;

			AllowDeactivation = true;
			EnableSpeculativeContacts = false;

			this.isParticle = isParticle;

			Update();
		}

        /// <summary>
        ///     If true, the body as no angular movement.
        /// </summary>
        public bool IsParticle {
			get => isParticle;
			set {
				if(isParticle && !value) {
					updatedHandler = ShapeUpdated;
					Shape.ShapeUpdated += updatedHandler;
					SetMassProperties();
					isParticle = false;
				} else if(!isParticle && value) {
					inertia = JMatrix.Zero;
					invInertia = invInertiaWorld = JMatrix.Zero;
					invOrientation = orientation = JMatrix.Identity;
					inverseMass = 1.0f;

					Shape.ShapeUpdated -= updatedHandler;

					isParticle = true;
				}

				Update();
			}
		}

		public ReadOnlyHashset<Arbiter> Arbiters { get; }

		public ReadOnlyHashset<Constraint> Constraints { get; }

        /// <summary>
        ///     If set to false the body will never be deactived by the
        ///     world.
        /// </summary>
        public bool AllowDeactivation { get; set; }

		public bool EnableSpeculativeContacts { get; set; }

        /// <summary>
        ///     Gets the current collision island the body is in.
        /// </summary>
        public CollisionIsland CollisionIsland => island;

        /// <summary>
        ///     If set to false the velocity is set to zero,
        ///     the body gets immediately freezed.
        /// </summary>
        public bool IsActive {
			get => isActive;
			set {
				if(!isActive && value)
					inactiveTime = 0.0f;
				else if(isActive && !value) {
					// if active and should be inactive
					inactiveTime = float.PositiveInfinity;
					angularVelocity = Vector3.Zero;
					linearVelocity = Vector3.Zero;
				}

				isActive = value;
			}
		}

        /// <summary>
        ///     Returns the torque which acts this timestep on the body.
        /// </summary>
        public Vector3 Torque => torque;

        /// <summary>
        ///     Returns the force which acts this timestep on the body.
        /// </summary>
        public Vector3 Force {
			get => force;
			set => force = value;
		}

        /// <summary>
        ///     Allows to set a user defined value to the body.
        /// </summary>
        public object Tag { get; set; }

        /// <summary>
        ///     The shape the body is using.
        /// </summary>
        public Shape Shape {
			get => shape;
			set {
				// deregister update event
				if(shape != null) shape.ShapeUpdated -= updatedHandler;

				// register new event
				shape = value;
				shape.ShapeUpdated += ShapeUpdated;
			}
		}

		public int BroadphaseTag { get; set; }

		public bool EnableDebugDraw {
			get => enableDebugDraw;
			set {
				enableDebugDraw = value;
				UpdateHullData();
			}
		}

        /// <summary>
        ///     The axis aligned bounding box of the body.
        /// </summary>
        public JBBox BoundingBox => boundingBox;


		public bool IsStaticOrInactive => !isActive || isStatic;

		public int CompareTo(RigidBody other) {
			if(other.instance < instance) return -1;
			if(other.instance > instance) return 1;
			return 0;
		}


		public void DebugDraw(IDebugDrawer drawer) {
			Vector3 pos1, pos2, pos3;

			for(var i = 0; i < hullPoints.Count; i += 3) {
				pos1 = hullPoints[i + 0];
				pos2 = hullPoints[i + 1];
				pos3 = hullPoints[i + 2];

				pos1 = pos1.Transform(ref orientation);
				pos1 = pos1 + position;

				pos2 = pos2.Transform(ref orientation);
				pos2 = pos2 + position;

				pos3 = pos3.Transform(ref orientation);
				pos3 = pos3 + position;

				drawer.DrawTriangle(pos1, pos2, pos3);
			}
		}

		public bool Equals(RigidBody other) => other.instance == instance;

        /// <summary>
        ///     Calculates a hashcode for this RigidBody.
        ///     The hashcode should be unique as possible
        ///     for every body.
        /// </summary>
        /// <returns>The hashcode.</returns>
        public override int GetHashCode() => hashCode;

		int CalculateHash(int a) {
			a = a ^ 61 ^ (a >> 16);
			a = a + (a << 3);
			a = a ^ (a >> 4);
			a = a * 0x27d4eb2d;
			a = a ^ (a >> 15);
			return a;
		}

        /// <summary>
        ///     Applies an impulse on the center of the body. Changing
        ///     linear velocity.
        /// </summary>
        /// <param name="impulse">Impulse direction and magnitude.</param>
        public void ApplyImpulse(Vector3 impulse) {
			if(isStatic)
				throw new InvalidOperationException("Can't apply an impulse to a static body.");

			Vector3 temp;
			temp = impulse * inverseMass;
			linearVelocity = linearVelocity + temp;
		}

        /// <summary>
        ///     Applies an impulse on the specific position. Changing linear
        ///     and angular velocity.
        /// </summary>
        /// <param name="impulse">Impulse direction and magnitude.</param>
        /// <param name="relativePosition">
        ///     The position where the impulse gets applied
        ///     in Body coordinate frame.
        /// </param>
        public void ApplyImpulse(Vector3 impulse, Vector3 relativePosition) {
			if(isStatic)
				throw new InvalidOperationException("Can't apply an impulse to a static body.");

			Vector3 temp;
			temp = impulse * inverseMass;
			linearVelocity = linearVelocity + temp;

			temp = Vector3.Cross(relativePosition, impulse);
			temp = temp.Transform(ref invInertiaWorld);
			angularVelocity = angularVelocity + temp;
		}

        /// <summary>
        ///     Adds a force to the center of the body. The force gets applied
        ///     the next time <see cref="World.Step" /> is called. The 'impact'
        ///     of the force depends on the time it is applied to a body - so
        ///     the timestep influences the energy added to the body.
        /// </summary>
        /// <param name="force">The force to add next <see cref="World.Step" />.</param>
        public void AddForce(Vector3 force) {
			this.force = force + this.force;
		}

        /// <summary>
        ///     Adds a force to the center of the body. The force gets applied
        ///     the next time <see cref="World.Step" /> is called. The 'impact'
        ///     of the force depends on the time it is applied to a body - so
        ///     the timestep influences the energy added to the body.
        /// </summary>
        /// <param name="force">The force to add next <see cref="World.Step" />.</param>
        /// <param name="pos">The position where the force is applied.</param>
        public void AddForce(Vector3 force, Vector3 pos) {
			this.force = this.force + force;
			pos = pos - position;
			pos = Vector3.Cross(pos, force);
			torque = pos + torque;
		}

        /// <summary>
        ///     Adds torque to the body. The torque gets applied
        ///     the next time <see cref="World.Step" /> is called. The 'impact'
        ///     of the torque depends on the time it is applied to a body - so
        ///     the timestep influences the energy added to the body.
        /// </summary>
        /// <param name="torque">The torque to add next <see cref="World.Step" />.</param>
        public void AddTorque(Vector3 torque) {
			this.torque = torque + this.torque;
		}

        /// <summary>
        ///     By calling this method the shape inertia and mass is used.
        /// </summary>
        public void SetMassProperties() {
			inertia = Shape.inertia;
			JMatrix.Inverse(ref inertia, out invInertia);
			inverseMass = 1.0f / Shape.mass;
			useShapeMassProperties = true;
		}

        /// <summary>
        ///     The engine used the given values for inertia and mass and ignores
        ///     the shape mass properties.
        /// </summary>
        /// <param name="inertia">The inertia/inverse inertia of the untransformed object.</param>
        /// <param name="mass">The mass/inverse mass of the object.</param>
        /// <param name="setAsInverseValues">
        ///     Sets the InverseInertia and the InverseMass
        ///     to this values.
        /// </param>
        public void SetMassProperties(JMatrix inertia, float mass, bool setAsInverseValues) {
			if(setAsInverseValues) {
				if(!isParticle) {
					invInertia = inertia;
					JMatrix.Inverse(ref inertia, out this.inertia);
				}

				inverseMass = mass;
			} else {
				if(!isParticle) {
					this.inertia = inertia;
					JMatrix.Inverse(ref inertia, out invInertia);
				}

				inverseMass = 1.0f / mass;
			}

			useShapeMassProperties = false;
			Update();
		}

		void ShapeUpdated() {
			if(useShapeMassProperties) SetMassProperties();
			Update();
			UpdateHullData();
		}

		public void SweptExpandBoundingBox(float timestep) {
			sweptDirection = linearVelocity * timestep;

			if(sweptDirection.X < 0.0f)
				boundingBox.Min.X += sweptDirection.X;
			else
				boundingBox.Max.X += sweptDirection.X;

			if(sweptDirection.Y < 0.0f)
				boundingBox.Min.Y += sweptDirection.Y;
			else
				boundingBox.Max.Y += sweptDirection.Y;

			if(sweptDirection.Z < 0.0f)
				boundingBox.Min.Z += sweptDirection.Z;
			else
				boundingBox.Max.Z += sweptDirection.Z;
		}

        /// <summary>
        ///     Recalculates the axis aligned bounding box and the inertia
        ///     values in world space.
        /// </summary>
        public virtual void Update() {
			if(isParticle) {
				inertia = JMatrix.Zero;
				invInertia = invInertiaWorld = JMatrix.Zero;
				invOrientation = orientation = JMatrix.Identity;
				boundingBox = shape.boundingBox;
				boundingBox.Min = boundingBox.Min + position;
				boundingBox.Max = boundingBox.Max + position;

				angularVelocity = Vector3.Zero;
			} else {
				// Given: Orientation, Inertia
				JMatrix.Transpose(ref orientation, out invOrientation);
				Shape.GetBoundingBox(ref orientation, out boundingBox);
				boundingBox.Min = boundingBox.Min + position;
				boundingBox.Max = boundingBox.Max + position;


				if(!isStatic) {
					JMatrix.Multiply(ref invOrientation, ref invInertia, out invInertiaWorld);
					JMatrix.Multiply(ref invInertiaWorld, ref orientation, out invInertiaWorld);
				}
			}
		}


		public virtual void PreStep(float timestep) {
			//
		}

		public virtual void PostStep(float timestep) {
			//
		}

		void UpdateHullData() {
			hullPoints.Clear();

			if(enableDebugDraw) shape.MakeHull(ref hullPoints, 3);
		}

		#region Properties

		public DampingType Damping { get; set; } = DampingType.Angular | DampingType.Linear;

		public Material Material {
			get => material;
			set => material = value;
		}

        /// <summary>
        ///     The inertia currently used for this body.
        /// </summary>
        public JMatrix Inertia => inertia;

        /// <summary>
        ///     The inverse inertia currently used for this body.
        /// </summary>
        public JMatrix InverseInertia => invInertia;

        /// <summary>
        ///     The velocity of the body.
        /// </summary>
        public Vector3 LinearVelocity {
			get => linearVelocity;
			set {
				if(isStatic)
					throw new InvalidOperationException("Can't set a velocity to a static body.");
				linearVelocity = value;
			}
		}

		// TODO: check here is static!
        /// <summary>
        ///     The angular velocity of the body.
        /// </summary>
        public Vector3 AngularVelocity {
			get => angularVelocity;
			set {
				if(isStatic)
					throw new InvalidOperationException("Can't set a velocity to a static body.");
				angularVelocity = value;
			}
		}

        /// <summary>
        ///     The current position of the body.
        /// </summary>
        public Vector3 Position {
			get => position;
			set {
				position = value;
				Update();
			}
		}

        /// <summary>
        ///     The current oriention of the body.
        /// </summary>
        public JMatrix Orientation {
			get => orientation;
			set {
				orientation = value;
				Update();
			}
		}

        /// <summary>
        ///     If set to true the body can't be moved.
        /// </summary>
        public bool IsStatic {
			get => isStatic;
			set {
				if(value && !isStatic) {
					if(island != null)
						island.islandManager.MakeBodyStatic(this);

					angularVelocity = Vector3.Zero;
					linearVelocity = Vector3.Zero;
				}

				isStatic = value;
			}
		}

		public bool AffectedByGravity {
			get => affectedByGravity;
			set => affectedByGravity = value;
		}

        /// <summary>
        ///     The inverse inertia tensor in world space.
        /// </summary>
        public JMatrix InverseInertiaWorld => invInertiaWorld;

        /// <summary>
        ///     Setting the mass automatically scales the inertia.
        ///     To set the mass indepedently from the mass use SetMassProperties.
        /// </summary>
        public float Mass {
			get => 1.0f / inverseMass;
			set {
				if(value <= 0.0f) throw new ArgumentException("Mass can't be less or equal zero.");

				// scale inertia
				if(!isParticle) {
					JMatrix.Multiply(ref Shape.inertia, value / Shape.mass, out inertia);
					JMatrix.Inverse(ref inertia, out invInertia);
				}

				inverseMass = 1.0f / value;
			}
		}

		#endregion
	}
}