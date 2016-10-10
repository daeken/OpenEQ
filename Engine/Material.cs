using System;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
    [Flags]
    public enum MaterialFlags {
        Masked = 1, 
        Translucent = 2, 
        Transparent = 4 
    }
    public class Material {
        MaterialFlags flags;
        Texture texture;
        public Material(MaterialFlags _flags, Texture _texture) {
            flags = _flags;
            texture = _texture;
        }

        public bool Enable() {
            GL.Enable(EnableCap.Blend);
            GL.BlendFunc(BlendingFactorSrc.SrcAlpha, BlendingFactorDest.OneMinusSrcAlpha);
            texture.Enable();

            return true;//(flags & MaterialFlags.Transparent) == 0;
        }
    }
}