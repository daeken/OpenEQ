using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenEQ.Engine {
    public class CharacterModel {
        Dictionary<string, float[][]> animations;
        Tuple<Material, uint[]>[] matpolys;

        public List<string> AnimationNames {
            get {
                return animations.Keys.ToList();
            }
        }
        string curAnimation = "";
        float animationStartTime;
        public string Animation {
            get { return curAnimation; }
            set {
                curAnimation = value;
                animationStartTime = Time.Now;
            }
        }
        public CharacterModel(Tuple<Material, uint[]>[] matpolys, Dictionary<string, float[][]> animations) {
            this.matpolys = matpolys;
            this.animations = animations;
            animationStartTime = Time.Now;
        }
    }
}
