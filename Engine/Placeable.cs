using System;
using OpenTK;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
    public class Placeable {
        Object obj;
        public Matrix4 Mat;
        public Placeable(Object _obj, Vector3 position, Vector3 rotation, Vector3 scale) {
            obj = _obj;
            Mat =
                Matrix4.CreateRotationX(rotation.X) * 
                Matrix4.CreateRotationY(rotation.Y) * 
                Matrix4.CreateRotationZ(rotation.Z) *
                Matrix4.CreateScale(scale) *  
                Matrix4.CreateTranslation(position);
        }
        public void Draw() {
            obj.Draw();
        }
    }
}