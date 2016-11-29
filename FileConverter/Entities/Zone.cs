using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using ICSharpCode.SharpZipLib.Zip;
using OpenEQ.FileConverter.Extensions;

namespace OpenEQ.FileConverter.Entities
{
    public class Zone
    {
        public ZoneObject ZoneObj;
        public List<ZoneObject> ZoneObjects;
        public Dictionary<string, ZoneObject> ZoneObjectsByName;
        public List<PlaceableObject> PlaceableObjects;

        public Zone()
        {
            // Initialize and create entry 0.  Entry 0 also becomes zoneobj
            ZoneObjects = new List<ZoneObject> {new ZoneObject()};
            ZoneObj = ZoneObjects[0];

            ZoneObjectsByName = new Dictionary<string, ZoneObject>();
            PlaceableObjects = new List<PlaceableObject>();
        }

        public ZoneObject AddObject(string name)
        {
            name = name.Replace("_DMSPRITEDEF", "");

            var zo = new ZoneObject(name);
            ZoneObjects.Add(zo);

            if (ZoneObjectsByName.ContainsKey(name))
            {
                ZoneObjectsByName[name] = zo;
            }
            else
            {
                ZoneObjectsByName.Add(name, zo);
            }
            return zo;
        }

        public void AddPlaceable(string objName, float[] position, float[] rotation, float[] scale)
        {
            if (!ZoneObjectsByName.ContainsKey(objName))
            {
                Console.WriteLine($"Could not place object {objName}.  It was not found in the Zone's list of objects.");
                return;
            }

            PlaceableObjects.Add(new PlaceableObject
            {
                ObjectInst = ZoneObjectsByName[objName],
                Position = position,
                Rotation = rotation,
                Scale = scale
            });
        }

    //    def coalesceObjectMeshes(self):
    //    for obj in self.objects:
    //        startmeshcount = len(obj.meshes)
    //        matmeshes = {}
    //        for mesh in obj.meshes:
    //            mat = mesh.material.filenames, mesh.material.flags
    //            if mat not in matmeshes:
    //                matmeshes[mat] = []
    //    matmeshes[mat].append(mesh)
    //        obj.meshes = []
    //    poss = 0
    //        for meshlist in matmeshes.values():
    //            if len(meshlist) == 1:
    //                obj.meshes.append(meshlist[0])
    //                continue
    //            gmesh = reduce(lambda a, b: a.add(b), meshlist)
    //            obj.meshes += gmesh.optimize()

        public void CoalesceObjectMeshes()
        {
            foreach (var obj in ZoneObjects)
            {
                var startMeshCount = obj.Meshes.Count;
                var matmeshes = new Dictionary<string, List<Mesh>>();
            
                foreach (var mesh in obj.Meshes)
                {
                    var mat = new Tuple<List<string>, int>(mesh.Material.filenames, mesh.Material.Flags);
                    var key = $"{string.Join(",", mesh.Material.filenames)}{mesh.Material.Flags}";
                    if (!matmeshes.ContainsKey(key))
                        matmeshes.Add(key, new List<Mesh>());

                    matmeshes[key].Add(mesh);
                }

                // We're done with it.
                obj.Meshes.Clear();
                var poss = 0;

                foreach (var meshlist in matmeshes.Values)
                {
                    if (1 == meshlist.Count)
                    {
                        obj.Meshes.Add(meshlist[0]);
                    }
                    else
                    {
                        var gmesh = meshlist[0];
                        for (var i = 1; i < meshlist.Count; i++)
                        {
                            //gmesh = reduce(lambda a, b: a.add(b), meshlist)
                            gmesh.Add(meshlist[i]);
                        }
                        obj.Meshes.AddRange(gmesh.Optimize());
                    }
                }
            }
        }

        public void Output(string outputFileName)
        {
            // Delete if it's already there.
            if (File.Exists(outputFileName))
                File.Delete(outputFileName);

            using (var fsOut = File.Create(outputFileName))
            {
                using (var zipArchive = new ZipArchive(fsOut, ZipArchiveMode.Create))
                {
                    CoalesceObjectMeshes();

                    var assets = new Dictionary<string, byte[]>();

                    foreach (var obj in ZoneObjects)
                    {
                        Console.WriteLine(obj.Name);
                        foreach (var mesh in obj.Meshes)
                        {
                            foreach (var fn in mesh.Material.filenames)
                            {
                                Console.WriteLine($"{mesh.Material.Flags}, {fn}");
                            }
                        }
                    }

                    foreach (var obj in ZoneObjects)
                    {
                        foreach (var mesh in obj.Meshes)
                        {
                            var material = mesh.Material;

                            for (var i = 0; i < material.filenames.Count; i++)
                            {
                                if (!assets.ContainsKey(material.filenames[i]))
                                {
                                    assets.Add(material.filenames[i], material.Textures[i]);
                                }
                            }
                        }
                    }
                    var aa = assets.OrderBy(a => a.Key);
                    // Skipping resample because I don't see anywhere it was ever set to true.
                    foreach (var asset in assets)
                    {
                        var zipEntry = zipArchive.CreateEntry(asset.Key, CompressionLevel.NoCompression);

                        using (var sw = zipEntry.Open())
                        {
                            sw.WriteBytes(asset.Value);
                            sw.Flush();
                            sw.Close();
                        }
                    }

                    foreach (var obj in ZoneObjects)
                    {
                        var optimizedMeshes = new List<Mesh>();
                        foreach (var mesh in obj.Meshes)
                        {
                            optimizedMeshes.AddRange(mesh.Optimize());
                        }
                        obj.Meshes = optimizedMeshes;
                    }

                    var zoneZipEntry = zipArchive.CreateEntry("zone.oez", CompressionLevel.NoCompression);
                    using (var sw = zoneZipEntry.Open())
                    {
                        var materials = new Dictionary<string, Tuple<int, Mesh>>();

                        foreach (var obj in ZoneObjects)
                        {
                            foreach (var mesh in obj.Meshes)
                            {
                                var key = $"{mesh.Material.Flags}{string.Join(",", mesh.Material.filenames)}";
                                if (!materials.ContainsKey(key))
                                    materials.Add(key, new Tuple<int, Mesh>(materials.Keys.Count, mesh));
                            }
                        }

                        sw.WriteBytes(BitConverter.GetBytes(materials.Keys.Count));

                        var orderedList = materials.Values.OrderBy(z => z.Item1).ToList();

                        foreach (var m in orderedList)
                        {
                            sw.WriteBytes(BitConverter.GetBytes(m.Item1));
                            sw.WriteBytes(BitConverter.GetBytes(m.Item2.Material.filenames.Count));
                            foreach (var fileName in m.Item2.Material.filenames)
                            {
                                sw.WriteString(fileName);
                            }
                        }

                        sw.WriteBytes(BitConverter.GetBytes(ZoneObjects.Count));

                        var objRefs = new Dictionary<ZoneObject, int>();
                        for (var i = 0; i < ZoneObjects.Count; i++)
                        {
                            objRefs.Add(ZoneObjects[i], i);
                            sw.WriteBytes(BitConverter.GetBytes(ZoneObjects[i].Meshes.Count));

                            foreach (var mesh in ZoneObjects[i].Meshes)
                            {
                                var matid =
                                    materials[$"{mesh.Material.Flags}{string.Join(",", mesh.Material.filenames)}"];
                                sw.WriteBytes(BitConverter.GetBytes(matid.Item1));
                                sw.WriteBytes(BitConverter.GetBytes(mesh.VertexBuffer.Count));
                                sw.WriteBytes(mesh.VertexBuffer.Data.ToArray());
                                sw.WriteBytes(BitConverter.GetBytes(mesh.Polygons.Count));
                                foreach (var poly in mesh.Polygons)
                                {
                                    sw.WriteBytes(poly.Item2.to_array());
                                }
                                foreach (var poly in mesh.Polygons)
                                {
                                    sw.WriteBytes(BitConverter.GetBytes((poly.Item1 ? 1 : 0)));
                                }
                            }
                        }

                        sw.WriteBytes(BitConverter.GetBytes(PlaceableObjects.Count));
                        foreach (var placeable in PlaceableObjects)
                        {
                            sw.WriteBytes(BitConverter.GetBytes(objRefs[placeable.ObjectInst]));
                            sw.WriteBytes(placeable.Position);
                            sw.WriteBytes(placeable.Rotation);
                            sw.WriteBytes(placeable.Scale);
                        }
                    }
                }
            }
        }

        //def output(self, zip):
    //    self.coalesceObjectMeshes()

    //    assets = {}
    //    for obj in self.objects:
    //        for mesh in obj.meshes:
    //            material = mesh.material
    //            for i, filename in enumerate(material.filenames):
    //                if filename not in assets:
    //                    assets[filename] = material.textures[i]
    //    if resample:
    //        print 'Resampling textures'
    //    for k, v in assets.items():
    //        if resample:
    //            v = resampleTexture(v)
    //            k = k.replace('.dds', '.png')
    //        zip.writestr(k, v)
    //    if resample:
    //        print 'Done'
        
    //    for obj in self.objects:
    //        obj.meshes = [x for m in obj.meshes for x in m.optimize()]

    //    def ouint(*x):
    //        zout.write(struct.pack('<' + 'I'*len(x), *x))
    //    def ofloat(*x):
    //        zout.write(struct.pack('<' + 'f'*len(x), *x))
    //    def ostring(x):
    //        sl = len(x)
    //        if sl == 0:
    //            zout.write(chr(0))
    //        while sl:
    //            zout.write(chr((sl & 0x7F) | (0x80 if sl > 127 else 0x00)))
    //            sl >>= 7
    //        zout.write(x)
    //    with tempfile.TemporaryFile() as zout:
    //        materials = {}
    //        for obj in self.objects:
    //            for mesh in obj.meshes:
    //                mat = mesh.material.flags, mesh.material.filenames
    //                if mat not in materials:
    //                    materials[mat] = len(materials)
    //        ouint(len(materials))
    //        for (flags, filenames), id in sorted(materials.items(), cmp=lambda a, b: cmp(a[1], b[1])):
    //            ouint(flags)
    //            ouint(len(filenames))
    //            for filename in filenames:
    //                ostring(filename)
    //        ouint(len(self.objects))
    //        objrefs = {}
    //        for i, obj in enumerate(self.objects):
    //            objrefs[obj] = i
    //            ouint(len(obj.meshes))
    //            for mesh in obj.meshes:
    //                matid = materials[(mesh.material.flags, mesh.material.filenames)]
    //                ouint(matid)
    //                ouint(len(mesh.vertbuffer))
    //                ofloat(*mesh.vertbuffer.data)
    //                ouint(len(mesh.polygons))
    //                for collidable, x in mesh.polygons:
    //                    ouint(*x)
    //                for collidable, x in mesh.polygons:
    //                    ouint(int(collidable))

    //        ouint(len(self.placeables))
    //        for placeable in self.placeables:
    //            ouint(objrefs[placeable.obj])
    //            ofloat(*placeable.position)
    //            ofloat(*placeable.rotation)
    //            ofloat(*placeable.scale)


    //        zout.seek(0)
    //        zip.writestr('zone.oez', zout.read())
    }
}
