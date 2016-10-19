using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenEQ.Engine {
    public class Mob {
        public CharacterModel Model;

        public Mob(CharacterModel model) {
            Model = model;
        }

        public void Draw(Program program) {
            Model.Draw(program);
        }
    }
}
