
namespace OpenEQ.FileConverter.Entities
{
    using System.IO;

    public class struct_wld_basic_frag
    {
        public uint size;
        public uint type;
        public int nameoff;

        public struct_wld_basic_frag(BinaryReader input)
        {
            size = input.ReadUInt32();
            type = input.ReadUInt32();
            nameoff = input.ReadInt32();
        }
    }
}