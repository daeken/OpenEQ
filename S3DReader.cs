using System;
using static System.Console;
using System.Diagnostics;
using System.IO;
using System.Collections.Generic;
using System.IO.Compression;
using static System.Text.Encoding;

namespace OpenEQ {
    public class S3DReader : IDisposable {
        Stream stream;
        BinaryReader reader;
        Dictionary<string, Tuple<uint, uint>> files;

        public List<string> Filenames {
            get {
                return new List<string>(files.Keys);
            }
        }

        public S3DReader(string path) {
            stream = File.Open(path, FileMode.Open);
            reader = new BinaryReader(stream);
            var offset = reader.ReadUInt32();
            Debug.Assert(reader.ReadUInt32() == 0x20534650);
            stream.Position = offset;
            
            var fileList = new List<Tuple<uint, uint>>();

            var count = reader.ReadUInt32();
            Stream dirstream = null;
            for(var i = 0; i < count; ++i) {
                stream.Position = offset + 4 + i * 12;
                uint crc = reader.ReadUInt32(), foff = reader.ReadUInt32(), size = reader.ReadUInt32();

                if(crc == 0x61580AC9) {
                    dirstream = OpenAt(foff, size);
                } else
                    fileList.Add(new Tuple<uint, uint>(foff, size));
            }
            fileList.Sort((a, b) => a.Item1.CompareTo(b.Item1));
            Debug.Assert(dirstream != null);

            var dreader = new BinaryReader(dirstream);
            Debug.Assert(fileList.Count == dreader.ReadUInt32());
            files = new Dictionary<string, Tuple<uint, uint>>();
            for(var i = 0; i < fileList.Count; ++i) {
                var fn = UTF8.GetString(dreader.ReadBytes(dreader.ReadInt32()-1));
                files[fn] = new Tuple<uint, uint>(fileList[i].Item1, fileList[i].Item2);
                dirstream.Position++;
            }
        }

        public Stream Open(string fn) {
            return OpenAt(files[fn].Item1, files[fn].Item2);
        }

        internal Stream OpenAt(uint pos, uint tsize) {
            var opos = stream.Position;
            stream.Position = pos;
            var outdata = new byte[tsize];
            uint tlen = 0;
            while(tlen < tsize) {
                var deflen = reader.ReadUInt32();
                var inflen = reader.ReadUInt32();
                var temp = stream.Position;
                stream.Position += 2;
                var dstream = new DeflateStream(stream, CompressionMode.Decompress);
                dstream.Read(outdata, (int) tlen, (int) inflen);
                stream.Position = temp + deflen;
                tlen += inflen;
            }
            stream.Position = opos;
            return new MemoryStream(outdata);
        }

        public void Dispose() {
            Dispose(true);
            GC.SuppressFinalize(this);
        }

        protected virtual void Dispose(bool disposing) {
            if(disposing) {
                reader.Dispose();
                stream.Dispose();
            }
        }
    } 
}
