using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using SiliconStudio.Core.Mathematics;
using SiliconStudio.Xenko.Input;
using SiliconStudio.Xenko.Engine;

namespace OpenEQ
{
    public class FPSCounter : SyncScript
    {
        public override void Update()
        {
            Game.Window.Title = $"OpenEQ (FPS: { Game.DrawTime.FramePerSecond })";
        }
    }
}
