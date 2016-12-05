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
    class OEQModelReader {
        public static Entity Read(Game game, string name) {
            var entity = new Entity(position: new Vector3(0, 0, 0), name: name + "Entity");

            return entity;
        }
    }
}