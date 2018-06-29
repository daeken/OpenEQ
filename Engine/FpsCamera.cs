using static System.Math;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class FpsCamera {
		public Vec3 Position;
		public double Pitch, Yaw;

		public static Mat4 Matrix;
		Mat3 LookRotation = Mat3.Identity;

		public FpsCamera(Vec3 pos) {
			Pitch = Yaw = 0;
			Position = pos;
			Update();
		}

		public void Move(Vec3 movement) {
			Position += LookRotation * movement;
		}

		public void Look(double pitchmod, double yawmod) {
			var eps = 0.0000001;
			Pitch = Clamp(Pitch + pitchmod, -PI / 2 + eps, PI / 2 - eps);
			Yaw += yawmod;
			LookRotation = Mat3.Pitch(Yaw) * Mat3.Roll(-Pitch);
		}

		public void Update() {
			var at = (Mat3.Pitch(Yaw) * Mat3.Roll(-Pitch) * vec3(0, 0, 1)).Normalized;
			Matrix = Mat4.LookAt(Position, Position + at, vec3(0, 1, 0));
		}
	}
}