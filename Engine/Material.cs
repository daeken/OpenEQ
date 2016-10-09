using System;

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

        public void Enable() {
            texture.Enable();
        }
    }
}