using System;
using static System.Console;
using System.Diagnostics;
using System.IO;
using static System.Text.Encoding;

namespace OpenEQ {
    public class WLDReader : IDisposable {
        Stream stream;
        BinaryReader reader;

        uint version;
        string stringTable;

        byte[] xorkey = {0x95, 0x3A, 0xC5, 0x2A, 0x95, 0x7A, 0x95, 0x6A};
        
        public WLDReader(Stream _stream) {
            stream = _stream;
            reader = new BinaryReader(_stream);

            Debug.Assert(reader.ReadUInt32() ==  0x54503D02);
            version = reader.ReadUInt32();
            var fragCount = reader.ReadUInt32();
            stream.Position += 8;
            var hashlen = reader.ReadInt32();
            stream.Position += 4;
            var hash = reader.ReadBytes(hashlen);
            for(var i = 0; i < hash.Length; ++i)
                hash[i] ^= xorkey[i % xorkey.Length];
            stringTable = UTF8.GetString(hash);

            for(var i = 0; i < fragCount; ++i) {
                var size = reader.ReadInt32();
                var type = reader.ReadInt32();
                var nameoff = reader.ReadInt32();
                var name = nameoff != -16777216 ? GetString(-Math.Min(nameoff, 0)) : "";
                switch(type) {
                    case 0x35: // First fragment
                        break;
                    /*case 0x21: // BSP Tree
                        Frag21(name);
                        break;*/
                    default:
                        stream.Position += size - 4;
                        WriteLine($"Unknown fragment: size={size} type={type:X02} name={name}");
                        break;
                }
            }

            Debug.Assert(reader.ReadUInt32() == 0xFFFFFFFF);
        }

        void Frag21(string name) {
            var count = reader.ReadUInt32();
            for(var i = 0; i < count; ++i) {
                var normal = reader.ReadVec3();
                var dist = reader.ReadSingle();
                var center = normal * dist;
                WriteLine($"{normal}*{dist} == {center}");
                var region = reader.ReadInt32();
                var left = reader.ReadInt32();
                var right = reader.ReadInt32();
            }
        }

        string GetString(int off) {
            for(int i = 0; i < stringTable.Length - off; ++i)
                if(stringTable[off + i] == '\0')
                    return stringTable.Substring(off, i);
            return "";
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