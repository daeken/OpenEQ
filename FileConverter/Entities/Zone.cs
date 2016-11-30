
namespace OpenEQ.FileConverter.Entities
{
    using System;
    using System.Collections.Generic;
    using System.IO;
    using System.IO.Compression;
    using System.Linq;
    using Extensions;

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

                    // Skipping resample because I don't see anywhere it was ever set to true.
                    foreach (var asset in assets)
                    {
                        var zipEntry = zipArchive.CreateEntry(asset.Key, CompressionLevel.NoCompression);

                        using (var bw = new BinaryWriter(zipEntry.Open()))
                        {
                            bw.Write(asset.Value);
                            bw.Flush();
                            bw.Close();
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
                    using (var bw = new BinaryWriter(zoneZipEntry.Open()))
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

                        bw.Write(materials.Keys.Count);

                        var orderedList = materials.Values.OrderBy(z => z.Item1).ToList();

                        foreach (var m in orderedList)
                        {
                            bw.Write(m.Item1);
                            bw.Write(m.Item2.Material.filenames.Count);
                            foreach (var fileName in m.Item2.Material.filenames)
                            {
                                bw.Write(fileName);
                            }
                        }

                        bw.Write(ZoneObjects.Count);

                        var objRefs = new Dictionary<ZoneObject, int>();
                        for (var i = 0; i < ZoneObjects.Count; i++)
                        {
                            objRefs.Add(ZoneObjects[i], i);
                            bw.Write(ZoneObjects[i].Meshes.Count);

                            foreach (var mesh in ZoneObjects[i].Meshes)
                            {
                                var matid =
                                    materials[$"{mesh.Material.Flags}{string.Join(",", mesh.Material.filenames)}"];
                                bw.Write(matid.Item1);
                                bw.Write(mesh.VertexBuffer.Count);
                                bw.WriteFloatArray(mesh.VertexBuffer.Data.ToArray());
                                bw.Write(mesh.Polygons.Count);
                                foreach (var poly in mesh.Polygons)
                                {
                                    bw.WriteFloatArray(poly.Item2.to_array());
                                }
                                foreach (var poly in mesh.Polygons)
                                {
                                    bw.Write(poly.Item1 ? 1 : 0);
                                }
                            }
                        }

                        bw.Write(PlaceableObjects.Count);
                        foreach (var placeable in PlaceableObjects)
                        {
                            bw.Write(objRefs[placeable.ObjectInst]);
                            bw.WriteFloatArray(placeable.Position);
                            bw.WriteFloatArray(placeable.Rotation);
                            bw.WriteFloatArray(placeable.Scale);
                        }
                    }
                }
            }
        }
    }
}