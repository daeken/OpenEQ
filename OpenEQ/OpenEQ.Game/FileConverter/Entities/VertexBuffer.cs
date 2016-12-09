using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using OpenEQ.FileConverter.Extensions;

namespace OpenEQ.FileConverter.Entities
{
    public class VertexBuffer
    {
        public List<float> Data;

        public int Count;

        public int Stride => Data.Count/Count;

        public IEnumerable<float> this[int i]
        {
            get
            {
                i *= 8;
                for (var j = i; j < i + 8; j++)
                {
                    yield return Data[j];
                }
            }
        }

        public VertexBuffer(FragMesh mesh)
        {
            Data = new List<float>();

            Count = mesh.Vertices.Length;

            // interleave and flatten into a list.
            Data = mesh.Vertices.InterleaveAndFlattenEqualLength(mesh.Normals, mesh.texcoords).ToList();
        }

        public VertexBuffer(List<float> data, int count)
        {
            Data = data;
            Count = count;
        }

        //    class VertexBuffer(object):
        //def __init__(self, data, count):
        //    self.data = data
        //    self.count = count
        //    assert len(data) % count == 0
        //    self.stride = len(data) / count

        //def __getitem__(self, index):
        //    return self.data[index * self.stride:(index + 1) * self.stride]

        //def __len__(self):
        //    return self.count
    }
}
