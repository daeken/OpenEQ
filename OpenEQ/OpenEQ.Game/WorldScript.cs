using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using SiliconStudio.Core.Mathematics;
using SiliconStudio.Xenko.Input;
using SiliconStudio.Xenko.Engine;
using OpenEQ.Network;

namespace OpenEQ {
    public class WorldScript : UIScript {
        internal WorldStream world;

        public override void Setup() {
            Console.WriteLine($"foo {world}");
        }

        public override void Update() {
        }
    }
}
