using System;
using static System.Console;
using System.Diagnostics;
using System.IO;
using static System.Text.Encoding;
using System.Collections.Generic;
using System.Linq;

namespace OpenEQ {
    public class WLDReader : IDisposable {
        public List<Vec3> Vertices, Normals;
        public List<Tuple<bool, int, int, int>> Polygons;

        Stream stream;
        BinaryReader reader;

        bool old;
        string stringTable;
        byte[] xorkey = {0x95, 0x3A, 0xC5, 0x2A, 0x95, 0x7A, 0x95, 0x6A};
        
        public WLDReader(Stream _stream) {
            Vertices = new List<Vec3>();
            Normals = new List<Vec3>();
            Polygons = new List<Tuple<bool, int, int, int>>();

            stream = _stream;
            reader = new BinaryReader(_stream);

            Debug.Assert(reader.ReadUInt32() ==  0x54503D02);
            old = reader.ReadUInt32() == 0x00015500;
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
                var epos = stream.Position + size - 4;
                switch(type) {
                    case 0x35: // First fragment
                        break;
                    /*case 0x21: // BSP Tree
                        Frag21(name);
                        break;*/
                    case 0x36: // Mesh
                        Frag36(name);
                        break;
                    default:
                        //WriteLine($"Unknown fragment: size={size} type={type:X02} name={name}");
                        break;
                }
                stream.Position = epos;
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

        void Frag36(string name) {
            var flags = reader.ReadUInt32();
            var tlistref = reader.ReadUInt32();
            var aniref = reader.ReadUInt32();
            stream.Position += 8; // Skip two fields
            var center = reader.ReadVec3();
            stream.Position += 12; // Skip three fields
            var maxdist = reader.ReadSingle();
            var min = reader.ReadVec3();
            var max = reader.ReadVec3();
            var vertcount = reader.ReadUInt16();
            var texcoordcount = reader.ReadUInt16();
            var normalcount = reader.ReadUInt16();
            var colorcount = reader.ReadUInt16();
            var polycount = reader.ReadUInt16();
            var vertpiececount = reader.ReadUInt16();
            var polytexcount = reader.ReadUInt16();
            var verttexcount = reader.ReadUInt16();
            var size9 = reader.ReadUInt16();
            float scale = 1 << reader.ReadUInt16();
            var verts = new Vec3[vertcount];
            for(var i = 0; i < vertcount; ++i)
                verts[i] = new Vec3(reader.ReadInt16() / scale, reader.ReadInt16() / scale, reader.ReadInt16() / scale) + center;
            var texcoords = new Tuple<int, int>[texcoordcount];
            for(var i = 0; i < texcoordcount; ++i)
                texcoords[i] = new Tuple<int, int>(old ? reader.ReadInt16() : reader.ReadInt32(), old ? reader.ReadInt16() : reader.ReadInt32());
            var normals = new Vec3[normalcount];
            for(var i = 0; i < normalcount; ++i)
                normals[i] = new Vec3(reader.ReadSByte() / 127f, reader.ReadSByte() / 127f, reader.ReadSByte() / 127f);
            var colors = new uint[colorcount];
            for(var i = 0; i < colorcount; ++i)
                colors[i] = reader.ReadUInt32();
            var polys = new Tuple<bool, int, int, int>[polycount]; // Collidable, vert1-3
            for(var i = 0; i < polycount; ++i)
                polys[i] = new Tuple<bool, int, int, int>(reader.ReadUInt16() != 0x0010, reader.ReadUInt16(), reader.ReadUInt16(), reader.ReadUInt16());
            stream.Position += 4 * vertpiececount; // Skip vertex piece stuff
            stream.Position += 4 * polytexcount; // Skip polygon texture IDs
            stream.Position += 4 * verttexcount; // Skip vertex texture IDs
            stream.Position += 12 * size9; // Skip animation

            var vstart = Vertices.Count;
            Vertices.AddRange(verts);
            Normals.AddRange(normals);
            Polygons.AddRange(polys.Select(v => new Tuple<bool, int, int, int>(v.Item1, v.Item2 + vstart, v.Item3 + vstart, v.Item4 + vstart)));
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