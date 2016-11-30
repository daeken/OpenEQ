
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
    }
}