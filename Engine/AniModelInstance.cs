using System.Numerics;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class AniModelInstance {
		public readonly AniModel Model;

		Matrix4x4 Transform = Matrix4x4.Identity;

		public Vector3 Position {
			get => Transform.Translation;
			set => Transform = Matrix4x4.CreateFromQuaternion(Rotation) * Matrix4x4.CreateTranslation(value);
		}

		Quaternion _Rotation;
		public Quaternion Rotation {
			get => _Rotation;
			set {
				_Rotation = value;
				Transform = Matrix4x4.CreateFromQuaternion(value) * Matrix4x4.CreateTranslation(Position);
			}
		}
		
		string _Animation = "C05";
		float AnimationStartTime = FrameTime;
		public string Animation {
			get => _Animation;
			set {
				_Animation = value;
				AnimationStartTime = FrameTime;
			}
		}

		public AniModelInstance(AniModel model) => Model = model;

		public void Draw(Matrix4x4 projView, bool forward) =>
			Model.Draw(projView, Transform, Animation,  FrameTime - AnimationStartTime, forward);
	}
}