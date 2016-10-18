using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using static System.Console;
using OpenEQ.Engine;
using OpenTK;
using System;

namespace OpenEQ {
    public class OEQCharReader {
        public static Dictionary<string, CharacterModel> Read(string path) {
            var outdict = new Dictionary<string, CharacterModel>();
            var zip = ZipFile.OpenRead(path);
            foreach(var ent in zip.Entries) {
                if(!ent.Name.EndsWith(".oec"))
                    continue;
                var charfile = ent.Open();
                var reader = new BinaryReader(charfile);

                var nummats = reader.ReadInt32();
                var matpolys = new Tuple<Material, uint[]>[nummats];
                for(var i = 0; i < nummats; ++i) {
                    var flags = reader.ReadUInt32();
                    var numtex = reader.ReadUInt32();
                    var textures = new Texture[numtex];
                    for(var j = 0; j < numtex; ++j) {
                        var fn = reader.ReadString();
                        var entry = zip.GetEntry(fn);
                        var zfp = new BinaryReader(entry.Open());
                        var faux = new MemoryStream(zfp.ReadBytes((int)entry.Length)); // Getting around the lack of seeking...
                        textures[j] = new Texture(faux);
                    }
                    var mat = new Material((MaterialFlags)flags, textures);
                    var numpoly = reader.ReadUInt32();
                    var idx = new uint[numpoly * 3];
                    for(var j = 0; j < numpoly * 3; ++j)
                        idx[j] = reader.ReadUInt32();
                    matpolys[i] = new Tuple<Material, uint[]>(mat, idx);
                }

                var numverts = reader.ReadUInt32();
                var animations = new Dictionary<string, float[][]>();
                var numani = reader.ReadUInt32();
                for(var i = 0; i < numani; ++i) {
                    var name = reader.ReadString();
                    var numframes = reader.ReadUInt32();
                    var cani = animations[name] = new float[numframes][];
                    for(var j = 0; j < numframes; ++j) {
                        cani[j] = new float[numverts * 8];
                        for(var k = 0; k < numverts * 8; ++k)
                            cani[j][k] = reader.ReadSingle();
                    }
                }

                outdict[ent.Name.Replace(".oec", "")] = new CharacterModel(matpolys, animations);
            }
            return outdict;
        }
    }
}