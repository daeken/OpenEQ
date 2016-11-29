
using System;
using System.Text;

namespace OpenEQ.FileConverter.Extensions
{
    using System.IO;

    public static class StreamExtensions
    {
        public static void WriteString(this Stream output, string data)
        {
            output.WriteByte((byte)data.Length);
            WriteBytes(output, Encoding.UTF8.GetBytes(data));
        }

        public static void WriteBytes(this Stream output, byte[] data)
        {
            output.Write(data, 0, data.Length);
        }

        public static void WriteBytes(this Stream output, float[] data)
        {
            var buf = new byte[sizeof(float)*data.Length];
            Buffer.BlockCopy(data, 0, buf, 0, buf.Length);
            output.Write(buf, 0, buf.Length);
        }
    }
}