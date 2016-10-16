using OpenTK;
using static System.Console;
using static System.Math;

namespace OpenEQ.Engine {
    public class Camera {
        Vector3 position;
        Vector2 rotation;

        public Matrix4 Matrix;
        public Vector3 Position {
            get { return position; }
            set {
                position = value;
                Update();
            }
        }

        public Camera(Vector3 pos) {
            Position = pos;
            rotation = new Vector2(0, 0);
        }

        public void Translate(Vector3 trans) {
            var strans = new Vector4(-trans.X, -trans.Z, trans.Y, 1.0f);
            strans = Vector4.Transform(strans, RotationMatrix().Inverted());
            Position += new Vector3(strans.X, strans.Y, strans.Z);
        }

        public void Rotate(Vector2 rot) {
            rotation += rot;
            rotation.Y = (float) Min(Max(rotation.Y, -PI / 2), PI / 2);
            Update();
        }

        Matrix4 RotationMatrix() {
            var rotx = Matrix4.CreateRotationX((float) -PI / 2 + rotation.Y);
            var roty = Matrix4.CreateRotationZ(rotation.X);
            return roty * rotx;
        }

        public void Update() {
            var trans = Matrix4.CreateTranslation(position);
            
            Matrix = trans * RotationMatrix();
        }
    }
}
