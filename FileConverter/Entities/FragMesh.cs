
namespace OpenEQ.FileConverter.Entities
{
    using System;
    using System.Collections.Generic;
    using System.IO;
    using GlmNet;
    using OpenEQ.FileConverter.Extensions;

    public class FragMesh
    {
        public string _name;
        public FragRef[] Textures;
        public vec3[] Vertices;
        public vec3[] Normals;
        public Tuple<bool, vec3>[] Polys;
        public List<ushort[]> BoneVertices;
        public List<ushort[]> Polytex;
        public List<float[]> texcoords;
        public uint[] Colors;

        public FragMesh(BinaryReader input, bool isOldVersion, FragRef[] textures, string name)
        {
            _name = name;

            texcoords = new List<float[]>();

            Textures = textures;

            input.BaseStream.Position += 8;

            var center = input.ReadSingle(3);

            input.BaseStream.Position += 12;

            // Skipping maxdist?
            input.ReadSingle();

            // Skipping min
            input.ReadSingle(3);//var min = new vec3(input.ReadSingle(), input.ReadSingle(), input.ReadSingle());

            // Skipping max
            input.ReadSingle(3);//var max = new vec3(input.ReadSingle(), input.ReadSingle(), input.ReadSingle());

            var vertcount = input.ReadUInt16();
            var texcoordcount = input.ReadUInt16();
            var normalcount = input.ReadUInt16();
            var colorcount = input.ReadUInt16();

            var polycount = input.ReadUInt16();
            var vertpiececount = input.ReadUInt16();
            var polytexcount = input.ReadUInt16();

            // Skipping verttexcount (yeah, 2 ts)?
            input.ReadUInt16();

            // Skipping size9?
            input.ReadUInt16();

            var scale = (float)(1 << input.ReadUInt16());

            Vertices = new vec3[vertcount];
            for (var i = 0; i < vertcount; i++)
            {
                Vertices[i] = new vec3(
                    input.ReadInt16() / scale + center[0],
                    input.ReadInt16() / scale + center[1],
                    input.ReadInt16() / scale + center[2]);
            }

            if (texcoordcount == 0)
                InitializeTexCoords(vertcount);
            else
                InitializeTexCoords(vertcount, input, isOldVersion);

            Normals = new vec3[normalcount];
            for (var i = 0; i < normalcount; i++)
            {
                Normals[i] = new vec3(
                    input.ReadSByte() / 127F,
                    input.ReadSByte() / 127F,
                    input.ReadSByte() / 127F);
            }

            Colors = input.ReadUInt32(colorcount);

            Polys = new Tuple<bool, vec3>[polycount];
            for (var i = 0; i < polycount; i++)
            {
                Polys[i] = new Tuple<bool, vec3>(input.ReadUInt16() != 0x0010,
                    new vec3(input.ReadUInt16(), input.ReadUInt16(), input.ReadUInt16()));
            }

            BoneVertices = new List<ushort[]>();
            for (var i = 0; i < vertpiececount; i++)
            {
                BoneVertices.Add(input.ReadUInt16(2));
            }

            Polytex = new List<ushort[]>();
            for (var i = 0; i < polytexcount; i++)
            {
                Polytex.Add(input.ReadUInt16(2));
            }
        }

        public void InitializeTexCoords(int count)
        {
            texcoords.Clear();

            for (var i = 0; i < count; i++)
            {
                texcoords.Add(new[] {0F, 0F});
            }
        }

        public void InitializeTexCoords(int count, BinaryReader input, bool isOldVersion)
        {
            if (isOldVersion)
                Internal_InitializeTexCoordsOld(count, input);
            else
                Internal_InitializeTexCoordsNew(count, input);
        }

        private void Internal_InitializeTexCoordsNew(int count, BinaryReader input)
        {
            for (var i = 0; i < count; i++)
            {
                texcoords.Add(input.ReadSingle(2));
            }
        }

        private void Internal_InitializeTexCoordsOld(int count, BinaryReader input)
        {
            for (var i = 0; i < count; i++)
            {
                texcoords.Add(new[]
                {
                    input.ReadInt16()/256F, input.ReadInt16()/256F
                });
            }
        }
    }
}