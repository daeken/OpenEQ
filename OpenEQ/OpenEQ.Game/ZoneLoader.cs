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
    public class ZoneLoader : AsyncScript
    {
        public string ZoneName;

        public override async Task Execute()
        {
            var task = new Task(() => {
                var zoneEntity = OEQZoneReader.Read((Game)Game, ZoneName);
                Entity.AddChild(zoneEntity);
            });
            task.Start();
            await task;
        }
    }
}
