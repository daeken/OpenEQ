
using System.Collections.Generic;

namespace OpenEQ.FileConverter.Entities
{
    public class ZoneObject
    {
        public string Name;

        public List<Mesh> Meshes;

        public ZoneObject(string name = "")
        {
            Name = name;
            Meshes = new List<Mesh>();
        }
    }
}