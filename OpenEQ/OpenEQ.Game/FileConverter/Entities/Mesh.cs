using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using GlmNet;

namespace OpenEQ.FileConverter.Entities
{
    public class Mesh
    {
        public Material Material;
        public VertexBuffer VertexBuffer;
        public List<Tuple<bool, vec3>> Polygons;


        public Mesh(Material material, VertexBuffer vbuf, List<Tuple<bool, vec3>> poly)
        {
            Material = material;
            VertexBuffer = vbuf;
            Polygons = poly;
        }

        public void Add(Mesh mesh)
        {
            var offset = VertexBuffer.Count;

            if (!VertexBuffer.Equals(mesh.VertexBuffer))
            {
                VertexBuffer.Data.AddRange(mesh.VertexBuffer.Data);
                VertexBuffer.Count += mesh.VertexBuffer.Count;

                foreach (var poly in mesh.Polygons)
                {
                    Polygons.Add(new Tuple<bool, vec3>(poly.Item1,
                        new vec3(poly.Item2.x + offset, poly.Item2.y + offset, poly.Item2.z + offset)));
                }
            }
            else
            {
                Polygons.AddRange(mesh.Polygons);
            }
        }

        private int MapIndex(float i, IDictionary<float, int> usedIndex, List<float> vbuffer)
        {
            if (usedIndex.ContainsKey(i))
            {
                return usedIndex[i];
            }

            var ind = usedIndex[i] = usedIndex.Keys.Count;
            vbuffer.AddRange(VertexBuffer[(int)i]);
            return ind;
        }

        public Mesh Subset(List<Tuple<bool, vec3>> cpoly)
        {
            var vbuffer = new List<float>();
            var npoly = new List<Tuple<bool, vec3>>();
            var used = new Dictionary<float, int>();

            foreach (var poly in cpoly)
            {
                npoly.Add(new Tuple<bool, vec3>(poly.Item1,
                    new vec3(MapIndex(poly.Item2.x, used, vbuffer),
                        MapIndex(poly.Item2.y, used, vbuffer), MapIndex(poly.Item2.z, used, vbuffer))));
            }

            return new Mesh(Material, new VertexBuffer(vbuffer, used.Count), npoly);
        }

        public List<Mesh> Optimize()
        {
            var outmeshes = new List<Mesh>();
            var cpoly = new List<Tuple<bool, vec3>>();
            
            foreach (var poly in Polygons)
            {
                    cpoly.Add(poly);
            }

            if (0 != cpoly.Count)
                outmeshes.Add(Subset(cpoly));

            return outmeshes;
        }
    }
}
