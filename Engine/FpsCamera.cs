using System;
using System.Linq;
using System.Numerics;
using Microsoft.Scripting.Utils;
using OpenEQ.Common;
using static System.MathF;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class FpsCamera {
		public Vector3 Position;
		float Pitch, Yaw;

		public static Matrix4x4 Matrix;
		public Matrix4x4 LookRotation = Matrix4x4.Identity;
		
		public static readonly Vector3 Up = new Vector3(0, 0, 1);
		public static readonly Vector3 Right = vec3(1, 0, 0);
		public static readonly Vector3 Forward = new Vector3(0, 1, 0);

		public float FallingVelocity;
		public bool OnGround;

		public const float CameraHeight = 7.5f;

		public FpsCamera(Vector3 pos) {
			Position = pos;
			Pitch = Yaw = 0;
		}

		public void Move(Vector3 movement) {
			if(movement.LengthSquared() < 0.0001) return;
			movement = Vector3.Transform(movement, LookRotation);
			if(PhysicsEnabled) {
				movement = ClipMovement(movement);
				// TODO: Figure out what a reasonable max is
				if(movement.Z > 1)
					movement.Z = 1;
				else if(movement.Z < -1)
					movement.Z = -1;
			}

			Position += movement;
		}

		Vector3 ClipMovement(Vector3 movement, int iterations = 3) {
			//Console.WriteLine($"Clipping {movement} {iterations}");
			const float padding = 2f;
			var moveLen = movement.Length();
			var moveDir = movement.Normalized();
			var hit = Collider.FindIntersection(Position, moveDir, 0.5f);
			if(hit == null) return movement;
			var dist = (hit.Value.Item2 - Position).Length();
			if(dist > moveLen + padding) return movement;
			if(iterations == 0) return moveDir * (dist - padding);
			var triNormal = hit.Value.Item1.Normal;
			var backoff = Vector3.Dot(movement, triNormal);
			movement -= triNormal * backoff;
			return ClipMovement(movement, iterations - 1);
		}

		public void Look(float pitchmod, float yawmod) {
			var eps = 0.01f;
			Pitch = clamp(Pitch + pitchmod, -PI / 2 + eps, PI / 2 - eps);
			Yaw += yawmod;
			LookRotation = Matrix4x4.CreateFromAxisAngle(Right, Pitch);
			if(Yaw != 0)
				LookRotation *= Matrix4x4.CreateFromAxisAngle(Up, Yaw);
		}

		public void Update(float timestep) {
			if(PhysicsEnabled) {
				if(FallingVelocity < 0)
					Position.Z -= FallingVelocity * timestep;

				Position.Z += 1 - CameraHeight;
				var downray = vec3(0.00001f, 0.00001f, -1).Normalized();
				var hit = Collider.FindIntersection(Position, downray, 0.5f);
				Position.Z -= 1 - CameraHeight;
				var dist = hit != null ? Position.Z - CameraHeight - hit.Value.Item2.Z : float.PositiveInfinity;
				var angle = hit != null ? Abs(Vector3.Dot(hit.Value.Item1.Normal, vec3(0, 0, 1))) : 1;
				OnGround = false;
				if(FallingVelocity >= 0 && dist < 1 && angle > 0.25f) {
					// Todo: Figure out what a reasonable default is
					Position.Z -= dist;
					FallingVelocity = 0;
					OnGround = true;
				} else {
					if(angle < 0.25f && hit != null) {
						var normal = hit.Value.Item1.Normal;
						Position.X += normal.X * timestep;
						Position.Y += normal.Y * timestep;
					}

					FallingVelocity += 250 * timestep;
					var delta = FallingVelocity * timestep;
					Position.Z -= Math.Min(delta, dist);
					if(delta > dist) {
						FallingVelocity = 0;
						OnGround = true;
					}
				}
			}

			var at = Vector3.Normalize(Vector3.Transform(Forward, LookRotation));
			Matrix = Matrix4x4.CreateLookAt(Position, Position + at, Up);
		}
	}
}