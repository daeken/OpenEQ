using System;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
    [Flags]
    public enum MaterialFlags {
        Masked = 1 << 0, 
        Translucent = 1 << 1, 
        Transparent = 1 << 2
    }
    public class Material {
        MaterialFlags flags;
        Texture texture;
        public Material(MaterialFlags _flags, Texture _texture) {
            flags = _flags;
            texture = _texture;
        }

        public bool Enable() {
            texture.Enable();

            return (flags & MaterialFlags.Transparent) == 0;
        }
    }
}