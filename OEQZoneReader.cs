using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using static System.Console;
using OpenEQ.Engine;

namespace OpenEQ {
    public class OEQZoneReader {
        public static List<Object> Read(string path) {
            var zip = ZipFile.OpenRead(path);
            var zonefile = zip.GetEntry("zone.oez").Open();
            var reader = new BinaryReader(zonefile);

            var nummats = reader.ReadInt32();
            var materials = new Dictionary<int, Material>();
            for(var i = 0; i < nummats; ++i) {
                var flags = reader.ReadUInt32();
                var fn = reader.ReadString();
                var entry = zip.GetEntry(fn);
                var zfp = new BinaryReader(entry.Open());
                var faux = new MemoryStream(zfp.ReadBytes((int) entry.Length)); // Getting around the lack of seeking...
                var tex = new Texture(faux);
                materials[i] = new Material((MaterialFlags) flags, tex);
            }

            var objects = new List<Object>();
            var numobjs = reader.ReadUInt32();
            for(var i = 0; i < numobjs; ++i) {
                var obj = new Object();
                objects.Add(obj);

                var name = reader.ReadString();
                var nummeshes = reader.ReadUInt32();
                for(var j = 0; j < nummeshes; ++j) {
                    var matid = reader.ReadInt32();
                    var numvert = reader.ReadInt32();
                    var vertbuffer = Enumerable.Range(0, numvert * 8).Select(_ => reader.ReadSingle()).ToArray();
                    var numpoly = reader.ReadInt32();
                    var indices = Enumerable.Range(0, numpoly * 3).Select(_ => (ushort) reader.ReadUInt32()).ToArray();
                    var collidable = Enumerable.Range(0, numpoly).Select(_ => reader.ReadUInt32() == 1).ToArray();

                    var mesh = new Mesh(materials[matid], vertbuffer, indices);
                    obj.AddMesh(mesh);
                }
            }

            return objects;
        }
    }
}