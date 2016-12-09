using System;
using System.Collections.Generic;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Threading.Tasks;

namespace OpenEQ.FileConverter.Entities
{
    public class Material
    {
        public int Flags;
        public List<string> filenames;
        public List<byte[]> Textures;

        public Material(int flags, List<byte[]> textures)
        {
            Flags = flags;
            filenames = new List<string>();
            foreach (var t in textures)
            {
                filenames.Add((BitConverter.ToString(SHA256.Create().ComputeHash(t)) + ".dds").ToLower().Replace("-", ""));
            }

            Textures = textures;
        }
    }
}
