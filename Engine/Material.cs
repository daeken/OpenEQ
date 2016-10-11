using System;
using static System.Console;

namespace OpenEQ.Engine {
    [Flags]
    public enum MaterialFlags {
        Masked = 1 << 0, 
        Translucent = 1 << 1, 
        Transparent = 1 << 2
    }
    public class Material {
        MaterialFlags flags;
        Texture[] textures;
        bool isStatic;
        const float frameDuration = 7.5f/60f;
        public Material(MaterialFlags _flags, Texture[] _textures) {
            flags = _flags;
            textures = _textures;
            isStatic = textures.Length <= 1;
        }

        public bool Enable() {
            if((flags & MaterialFlags.Transparent) != 0)
                return false;
            
            if(isStatic)
                textures[0].Enable();
            else
                textures[(int) (Time.Now / frameDuration) % textures.Length].Enable();

            return true;
        }
    }
}