namespace OpenEQ.FileConverter.Entities
{
    using System;
    using System.Collections.Generic;
    using System.IO;
    using Ionic.Zip;
    using System.Linq;
    using Extensions;
    using OpenEQ;
    using SiliconStudio.Xenko.Graphics;

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
                            gmesh.Add(meshlist[i]);
                        }
                        obj.Meshes.AddRange(gmesh.Optimize());
                    }
                }
            }
        }

        public void Output(Stream fsOut) {
            var zipArchive = new ZipFile();
            CoalesceObjectMeshes();

            var assets = new Dictionary<string, byte[]>();
            var repl = new Dictionary<string, Tuple<string, byte[]>>();

            foreach(var obj in ZoneObjects) {
                foreach(var mesh in obj.Meshes) {
                    var material = mesh.Material;

                    for(var i = 0; i < material.filenames.Count; i++) {
                        var ofn = material.filenames[i];
                        if(!assets.ContainsKey(ofn))
                            assets.Add(material.filenames[i], material.Textures[i]);
                    }
                }
            }

            foreach(var tu in assets.AsParallel().Select(kv => {
                var xenkofn = kv.Key.Substring(0, kv.Key.Length - 4) + ImageFileType.Xenko.ToFileExtension();
                var tdata = TextureLoader.Convert(kv.Value);
                if(tdata == null)
                    return null;
                return new Tuple<string, string, byte[]>(kv.Key, xenkofn, tdata);
            })) {
                if(tu != null)
                    repl[tu.Item1] = new Tuple<string, byte[]>(tu.Item2, tu.Item3);
            }

            foreach(var asset in assets)
                if(repl.ContainsKey(asset.Key))
                    zipArchive.AddEntry(repl[asset.Key].Item1, repl[asset.Key].Item2);
                else
                    zipArchive.AddEntry(asset.Key, asset.Value);

            foreach(var obj in ZoneObjects) {
                var optimizedMeshes = new List<Mesh>();
                foreach(var mesh in obj.Meshes) {
                    optimizedMeshes.AddRange(mesh.Optimize());
                }
                obj.Meshes = optimizedMeshes;
            }

            using(var ms = new MemoryStream()) {
                using(var bw = new BinaryWriter(ms)) {
                    var materials = new Dictionary<string, Tuple<int, int, Mesh>>();

                    foreach(var obj in ZoneObjects) {
                        foreach(var mesh in obj.Meshes) {
                            var key = $"{mesh.Material.Flags}{string.Join(",", mesh.Material.filenames)}";
                            if(!materials.ContainsKey(key))
                                materials.Add(key, new Tuple<int, int, Mesh>(materials.Keys.Count, mesh.Material.Flags, mesh));
                        }
                    }

                    bw.Write(materials.Keys.Count);

                    var orderedList = materials.Values.OrderBy(z => z.Item1).ToList();

                    foreach(var m in orderedList) {
                        bw.Write(m.Item2);
                        bw.Write(m.Item3.Material.filenames.Count);
                        foreach(var fileName in m.Item3.Material.filenames) {
                            bw.Write(repl.ContainsKey(fileName) ? repl[fileName].Item1 : fileName);
                        }
                    }

                    bw.Write(ZoneObjects.Count);

                    var objRefs = new Dictionary<ZoneObject, int>();
                    for(var i = 0; i < ZoneObjects.Count; i++) {
                        objRefs.Add(ZoneObjects[i], i);
                        bw.Write(ZoneObjects[i].Meshes.Count);

                        foreach(var mesh in ZoneObjects[i].Meshes) {
                            var matid =
                                materials[$"{mesh.Material.Flags}{string.Join(",", mesh.Material.filenames)}"];
                            bw.Write(matid.Item1);
                            bw.Write(mesh.VertexBuffer.Count);
                            bw.WriteFloatArray(mesh.VertexBuffer.Data.ToArray());
                            bw.Write(mesh.Polygons.Count);
                            foreach(var poly in mesh.Polygons) {
                                bw.WriteIntArray(poly.Item2.to_array());
                            }
                            foreach(var poly in mesh.Polygons) {
                                bw.Write(poly.Item1 ? 1 : 0);
                            }
                        }
                    }

                    bw.Write(PlaceableObjects.Count);
                    foreach(var placeable in PlaceableObjects) {
                        bw.Write(objRefs[placeable.ObjectInst]);
                        bw.WriteFloatArray(placeable.Position);
                        bw.WriteFloatArray(placeable.Rotation);
                        bw.WriteFloatArray(placeable.Scale);
                    }
                }
                ms.Flush();
                zipArchive.AddEntry("zone.oez", ms.GetBuffer());
            }

            zipArchive.Save(fsOut);
        }
    }
}