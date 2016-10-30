using LibRocketNet;
using OpenEQ.GUI.MoonRocket;
using System.Text.RegularExpressions;
using static System.Console;

namespace OpenEQ.GUI.MoonRocket {
    public class MoonListener : EventListener {
        CoreMoonRocket moonRocket;
        string code;
        Element element;
        bool isBareRef;

        public MoonListener(CoreMoonRocket moonRocket, string code, Element element) {
            this.moonRocket = moonRocket;
            this.code = code;
            this.element = element;

            isBareRef = IsFunctionReference(code);
        }

        bool IsFunctionReference(string code) {
            var parts = code.Split('.', ':');
            foreach(var part in parts) {
                if(!Regex.IsMatch(part, @"^[a-zA-Z_][a-zA-Z_0-9]*$"))
                    return false;
            }
            return true;
        }

        public override unsafe void ProcessEvent(ElementEventArgs e) {
            moonRocket.ProcessScriptEvent(element, code, e, isBareRef);
        }
    }

    class MoonListenerInstancer : EventListenerInstancer {
        CoreMoonRocket moonRocket;
        public MoonListenerInstancer(CoreMoonRocket moonRocket) {
            this.moonRocket = moonRocket;
        }

        public override EventListener InstanceEventListener(string value, Element element) {
            return new MoonListener(moonRocket, value, element);
        }
    }
}
