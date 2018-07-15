using System.Numerics;
using OpenEQ.Common;
using static System.MathF;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class FpsCamera {
		public Vector3 Position;
		public float Pitch, Yaw;

		public static Matrix4x4 Matrix;
		public Matrix4x4 LookRotation = Matrix4x4.Identity;
		
		static readonly Vector3 Up = new Vector3(0, 0, 1);
		static readonly Vector3 Right = vec3(1, 0, 0);
		static readonly Vector3 Forward = new Vector3(0, 1, 0);

		public FpsCamera(Vector3 pos) {
			Pitch = Yaw = 0;
			Position = pos;
			Update();
		}

		public void Move(Vector3 movement) {
			Position += Vector3.Transform(movement, LookRotation);
		}

		public void Look(float pitchmod, float yawmod) {
			var eps = 0.0000001f;
			Pitch = clamp(Pitch + pitchmod, -PI / 2 + eps, PI / 2 - eps);
			Yaw += yawmod;
			LookRotation = Matrix4x4.CreateFromAxisAngle(Right, Pitch) * Matrix4x4.CreateFromAxisAngle(Up, Yaw);
		}

		public void Update() {
			var at = Vector3.Normalize(Vector3.Transform(Forward, LookRotation));
			Matrix = Matrix4x4.CreateLookAt(Position, Position + at, Up);
		}
	}
}