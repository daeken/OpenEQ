using SiliconStudio.Core.Mathematics;
using SiliconStudio.Xenko.Graphics;
using SiliconStudio.Xenko.Rendering;
using System.IO;

namespace OpenEQ {
    static class Extensions {
        public static Material Clone(this Material material, GraphicsDevice device) {
            var nmat = new Material();
            nmat.Descriptor = material.Descriptor;
            nmat.TessellationMethod = material.TessellationMethod;
            nmat.HasTransparency = material.HasTransparency;
            nmat.IsLightDependent = material.IsLightDependent;
            nmat.Parameters = new ParameterCollection(material.Parameters);
            return nmat;
        }

        public static Vector2 ReadVector2(this BinaryReader br) {
            return new Vector2(br.ReadSingle(), br.ReadSingle());
        }
        public static Vector3 ReadVector3(this BinaryReader br) {
            return new Vector3(br.ReadSingle(), br.ReadSingle(), br.ReadSingle());
        }
    }
}
