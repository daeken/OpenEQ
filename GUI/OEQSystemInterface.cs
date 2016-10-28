using System;
using LibRocketNet;

namespace OpenEQ.GUI {
    public class OEQSystemInterface : SystemInterface {
        protected override float GetElapsedTime() {
            return Time.Now;
        }
    }
}
