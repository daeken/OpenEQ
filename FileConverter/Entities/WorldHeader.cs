
namespace OpenEQ.FileConverter.Entities
{
    using System.IO;

    public struct WorldHeader
    {
        public int magic;
        public int version;
        public int fragmentCount;
        public int header3;
        public int header4;
        public int stringHashSize;
        public int header6;

        public bool IsOldVersion => 0x00015500 == version;

        public WorldHeader(BinaryReader input)
        {
            magic = input.ReadInt32();
            version = input.ReadInt32();
            fragmentCount = input.ReadInt32();
            header3 = input.ReadInt32();
            header4 = input.ReadInt32();
            stringHashSize = input.ReadInt32();
            header6 = input.ReadInt32();
        }
    }
}