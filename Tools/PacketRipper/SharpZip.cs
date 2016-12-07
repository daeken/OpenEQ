
namespace PacketRipper
{
    using System.IO;
    using ICSharpCode.SharpZipLib.Zip.Compression;
    using ICSharpCode.SharpZipLib.Zip.Compression.Streams;

    public class SharpZip
    {
        public static int InflatePacket(byte[] indata, int inoffset, int indatalen, byte[] outdata, int outoffset)
        {
            using (var ms = new MemoryStream(indata, inoffset, indatalen))
            {
                var inflater = new Inflater(false);
                var inStream = new InflaterInputStream(ms, inflater);

                // Copy the decompressed data.
                return inStream.Read(outdata, outoffset, outdata.Length - outoffset);
            }
        }

        public static int DeflatePacket(byte[] indata, int inoffset, int indatalen, byte[] outdata, int outoffset)
        {
            using (var ms = new MemoryStream())
            {
                var deflater = new Deflater(Deflater.DEFAULT_COMPRESSION, false);
                using (var outStream = new DeflaterOutputStream(ms, deflater))
                {
                    outStream.IsStreamOwner = false;
                    outStream.Write(indata, inoffset, indatalen);
                    outStream.Flush();
                    outStream.Finish();
                }

                // Copy the data.
                ms.Position = 0;
                ms.Read(outdata, outoffset, (int)ms.Length);

                return (int)ms.Length;
            }
        }
    }
}
