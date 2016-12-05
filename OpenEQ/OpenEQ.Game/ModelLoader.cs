using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using SiliconStudio.Core.Mathematics;
using SiliconStudio.Xenko.Input;
using SiliconStudio.Xenko.Engine;

namespace OpenEQ {
    public class ModelLoader : AsyncScript {
        public string ModelName;

        public override async Task Execute() {
            var task = new Task(() => {
                var modelEntity = OEQModelReader.Read((Game) Game, ModelName);
                Entity.AddChild(modelEntity);
            });
            task.Start();
            await task;
        }
    }
}
