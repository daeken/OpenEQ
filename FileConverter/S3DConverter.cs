
namespace OpenEQ.FileConverter
{
    using System;
    using System.Collections.Generic;
    using System.IO;
    using System.IO.Compression;
    using System.Linq;
    using System.Threading.Tasks;
    using Extensions;

    public class S3DConverter
    {
        public static async Task<Dictionary<string, byte[]>> Convert(string fileName)
        {
            var directoryList = new List<string>();
            var outDict = new Dictionary<string, byte[]>();

            await Task.Run(() =>
            {
                using (var fs = File.OpenRead(fileName))
                {
                    using (var input = new BinaryReader(fs))
                    {
                        var offset = input.ReadUInt32();

                        if ("PFS " != string.Join("", input.ReadChars(4)))
                        {
                            throw new FormatException("Expected header tag (PFS ) was not found.");
                        }

                        input.BaseStream.Position = offset;

                        var count = input.ReadUInt32();
                        var fileList = new List<Tuple<uint, byte[]>>();

                        for (var i = 0; i < count; i++)
                        {
                            input.BaseStream.Position = offset + sizeof(int) + i * 12;
                            var crc = input.ReadUInt32();
                            var foff = input.ReadUInt32();
                            var size = input.ReadUInt32();

                            input.BaseStream.Position = foff;

                            var tpos = 0;
                            var outdata = new byte[size];

                            while (tpos < size)
                            {
                                var deflen = input.ReadUInt32();
                                var inflen = input.ReadUInt32();

                                var tempPosition = input.BaseStream.Position;
                                input.BaseStream.Position += 2; // wtf?
                                using (var dstream = new DeflateStream(input.BaseStream, CompressionMode.Decompress, true))
                                {
                                    dstream.Read(outdata, tpos, (int)inflen);
                                }
                                input.BaseStream.Position = tempPosition + deflen;
                                tpos += (int)inflen;
                            }

                            if (crc == 0x61580AC9)
                                directoryList = ParseDirectory(outdata);
                            else
                                fileList.Add(new Tuple<uint, byte[]>(foff, outdata));
                        }

                        // Sort by offset.
                        fileList = fileList.OrderBy(a => a.Item1).ToList();

                        // Now finish building the File list by taking the data for each file
                        // and assigning it to the file entry.
                        for (var i = 0; i < directoryList.Count(); i++)
                        {
                            outDict.Add(directoryList[i], fileList[i].Item2);
                        }
                    }
                }
            });

            return outDict;
        }

        private static List<string> ParseDirectory(byte[] directory)
        {
            var files = new List<string>();

            using (var br = new BinaryReader(new MemoryStream(directory)))
            {
                var totalFiles = br.ReadInt32();

                // Yes, I'm trusting that the total files count isn't bad, but I'm not really
                // concerned about it in this case.  If the user's files are jacked up, that's
                // their own fault.
                for (var i = 0; i < totalFiles; i++)
                {
                    files.Add(br.ReadString(br.ReadInt32()));
                }
            }

            return files;
        }
    }
}
