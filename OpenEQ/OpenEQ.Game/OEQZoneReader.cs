using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using SiliconStudio.Core.IO;
using SiliconStudio.Xenko.Engine;
using SiliconStudio.Core.Mathematics;
using SiliconStudio.Xenko.Rendering;
using SiliconStudio.Xenko.Graphics;
using SiliconStudio.Xenko.Rendering.Materials;

namespace OpenEQ {
    class OEQZoneReader {
        public static Entity Read(Game game, string name) {
            var zip = new ZipArchive(VirtualFileSystem.OpenStream($"/cache/{name}.zip", VirtualFileMode.Open, VirtualFileAccess.Read));
            var zonefile = zip.GetEntry("zone.oez").Open();
            var reader = new BinaryReader(zonefile);

            var entity = new Entity(position: new Vector3(0, 0, 0), name: "ZoneEntity");

            var nummats = reader.ReadInt32();
            var materials = new Dictionary<int, Material>();
            var hidden = new Dictionary<int, bool>();
            for(var i = 0; i < nummats; ++i) {
                var flags = reader.ReadUInt32();
                var numtex = reader.ReadUInt32();
                var textures = new Texture[numtex];
                for(var j = 0; j < numtex; ++j) {
                    var fn = reader.ReadString();
                    var entry = zip.GetEntry(fn);
                    var zfp = new BinaryReader(entry.Open());
                    var data = zfp.ReadBytes((int)entry.Length);
                    var img = Image.Load(data);
                    textures[j] = Texture.New(game.GraphicsDevice, img);
                }
                hidden[i] = flags == 4;
                var matname = "DiffuseMaterial";
                if(flags == 1)
                    matname = "DiffuseMaskedMaterial";
                else if(flags != 0)
                    matname = "DiffuseTranslucentMaterial";
                var mat = materials[i] = game.Content.Load<Material>(matname).Clone(game.GraphicsDevice);
                mat.Parameters.Set(TexturingKeys.Sampler, game.GraphicsDevice.SamplerStates.AnisotropicWrap);
                mat.Parameters.Set(MaterialKeys.DiffuseMap, textures[0]);
            }

            var objects = new List<Model>();
            var numobjs = reader.ReadUInt32();
            for(var i = 0; i < numobjs; ++i) {
                var obj = new Model();
                objects.Add(obj);
                var matoffs = new List<int>();

                var nummeshes = reader.ReadUInt32();
                for(var j = 0; j < nummeshes; ++j) {
                    var matid = reader.ReadInt32();
                    var numvert = reader.ReadInt32();
                    var verts = Enumerable.Range(0, numvert).Select(_ => new VertexPositionNormalTexture(reader.ReadVector3(), reader.ReadVector3(), reader.ReadVector2())).ToArray();
                    var numpoly = reader.ReadInt32();
                    var indices = Enumerable.Range(0, numpoly * 3).Select(_ => (int) reader.ReadUInt32()).ToArray();
                    var collidable = Enumerable.Range(0, numpoly).Select(_ => reader.ReadUInt32() == 1).ToArray();

                    if(hidden[matid])
                        continue;
                    var vertbuffer = Buffer.Vertex.New(game.GraphicsDevice, verts);
                    var indexbuffer = Buffer.Index.New(game.GraphicsDevice, indices);

                    var md = new MeshDraw {
                        PrimitiveType = PrimitiveType.TriangleList,
                        VertexBuffers = new[] { new VertexBufferBinding(vertbuffer, VertexPositionNormalTexture.Layout, vertbuffer.ElementCount) }, 
                        IndexBuffer = new IndexBufferBinding(indexbuffer, true, indexbuffer.ElementCount), 
                        DrawCount = indexbuffer.ElementCount
                    };

                    var mesh = new Mesh() {
                        Draw = md
                    };
                    if(matoffs.Contains(matid))
                        mesh.MaterialIndex = matoffs.IndexOf(matid);
                    else {
                        mesh.MaterialIndex = matoffs.Count;
                        matoffs.Add(matid);
                        obj.Materials.Add(materials[matid]);
                    }
                    obj.Add(mesh);
                }
            }

            var component = new ModelComponent(objects[0]);
            entity.Add(component);
            var numplace = reader.ReadUInt32();
            for(var i = 0; i < numplace; ++i) {
                component = new ModelComponent(objects[reader.ReadInt32()]);
                var subent = new Entity();
                subent.Transform.Position = reader.ReadVector3();
                subent.Transform.RotationEulerXYZ = reader.ReadVector3();
                subent.Transform.Scale = reader.ReadVector3();
                subent.Add(component);
                entity.AddChild(subent);
            }

            return entity;
        }
    }
}