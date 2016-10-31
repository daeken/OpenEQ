using System;
using LibRocketNet;
using static System.Console;

namespace OpenEQ.GUI.MoonRocket {
    class ScriptNodeHandler : XMLNodeHandler {
        Element currentScript;

        public override bool ElementData(string data) {
            currentScript.AppendChild(currentScript.OwnerDocument.CreateTextNode(data));
            return true;
        }

        public override bool ElementEnd(string name) {
            return true;
        }

        public override Element ElementStart(Element parent, string name) {
            currentScript = parent.OwnerDocument.CreateElement(name);
            parent.AppendChild(currentScript);
            return currentScript;
        }
    }
}
