
namespace OpenEQ.FileConverter.Extensions
{
    using System.IO;

    public static class BinaryWriterExtensions
    {
        public static void WriteFloatArray(this BinaryWriter output, float[] data)
        {
            foreach (var f in data)
            {
                output.Write(f);
            }
        }

        public static void WriteIntArray(this BinaryWriter output, float[] data)
        {
            foreach (var f in data)
            {
                output.Write((int)f);
            }
        }
    }
}