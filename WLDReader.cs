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
        public List<Tuple<float, float>> TexCoords;
        public Dictionary<WldMaterial, List<Tuple<bool, int, int, int>>> PolygonsByMaterial;

        List<Tuple<bool, int, int, int, int, int>> Polygons;
        Dictionary<int, int[]> Frag31Map; // Arrays of frag references
        Dictionary<int, Tuple<uint, int>> Frag30Map; // Flags * reference to 0x05

        Stream stream;
        BinaryReader reader;

        bool old;
        string stringTable;
        byte[] xorkey = {0x95, 0x3A, 0xC5, 0x2A, 0x95, 0x7A, 0x95, 0x6A};
        
        public WLDReader(Stream _stream) {
            Vertices = new List<Vec3>();
            Normals = new List<Vec3>();
            TexCoords = new List<Tuple<float, float>>();
            Polygons = new List<Tuple<bool, int, int, int, int, int>>();

            Frag31Map = new Dictionary<int, int[]>();

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
                if(name == "")
                    WriteLine($"null name of type {type:X02}");
                var epos = stream.Position + size - 4;
                switch(type) {
                    case 0x35: // First fragment
                        break;
                    case 0x30: // Texture
                        Frag30(i);
                        break;
                    case 0x31: // Texture list
                        Frag31(i);
                        break;
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

        void Frag30(int id) {
            var start = stream.Position;
            var pairflags = reader.ReadUInt32();
            var flags = reader.ReadUInt32();
            stream.Position += 12;
            if((pairflags & 2) == 2)
                stream.Position += 8;
            var refid = reader.ReadInt32();
        }

        void Frag31(int id) {
            stream.Position += 4;
            var size = reader.ReadUInt32();
            var list = new int[size];
            for(var i = 0; i < size; ++i)
                list[i] = reader.ReadInt32();
            Frag31Map[id] = list;
            WriteLine(id);
        }

        void Frag36(string name) {
            var flags = reader.ReadUInt32();
            var tlistref = reader.ReadInt32();
            if(!Frag31Map.ContainsKey(tlistref - 1))
                WriteLine("Foo");
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
            var texcoords = new Tuple<float, float>[texcoordcount];
            if(texcoordcount == 0) {
                texcoords = new Tuple<float, float>[vertcount];
                for(var i = 0; i < vertcount; ++i)
                    texcoords[i] = new Tuple<float, float>(0, 0);
            } else {
                for(var i = 0; i < texcoordcount; ++i)
                    texcoords[i] = new Tuple<float, float>(old ? reader.ReadInt16() : reader.ReadInt32(), old ? reader.ReadInt16() : reader.ReadInt32());
            }
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
            var polytex = new int[polycount];
            var off = 0;
            for(var i = 0; i < polytexcount; ++i) {
                var count = reader.ReadUInt16();
                var texindex = reader.ReadUInt16();
                for(var j = 0; j < count; ++j)
                    polytex[off++] = texindex;
            }
            stream.Position += 4 * verttexcount; // Skip vertex texture IDs
            stream.Position += 12 * size9; // Skip animation

            var vstart = Vertices.Count;
            Vertices.AddRange(verts);
            Normals.AddRange(normals);
            TexCoords.AddRange(texcoords);
            Polygons.AddRange(polys.Select((v, i) => new Tuple<bool, int, int, int, int, int>(v.Item1, v.Item2 + vstart, v.Item3 + vstart, v.Item4 + vstart, tlistref, polytex[i])));
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