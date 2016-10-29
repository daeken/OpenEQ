using LibRocketNet;
using OpenEQ.GUI.MoonRocket;
using static System.Console;

namespace OpenEQ.GUI.MoonRocket {
    public class MoonListener : EventListener {
        CoreMoonRocket moonRocket;
        string code;
        Element element;

        public MoonListener(CoreMoonRocket moonRocket, string code, Element element) {
            this.moonRocket = moonRocket;
            this.code = code;
            this.element = element;
        }

        public override unsafe void ProcessEvent(ElementEventArgs e) {
            moonRocket.ProcessScriptEvent(element, code, e);
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
