using LibRocketNet;
using MoonSharp;
using static System.Console;

namespace OpenEQ.GUI.MoonRocket {
    public class MoonRocketPlugin : Plugin {
        CoreMoonRocket moonRocket;

        public MoonRocketPlugin(CoreMoonRocket moonRocket) {
            this.moonRocket = moonRocket;
        }
        public override void OnInitialise() {
        }
        public override void OnShutdown() {
        }

        public override void OnContextCreate(Context context) {
        }

        public override void OnContextDestroy(Context context) {
        }

        public override void OnDocumentOpen(Context context, string path) {
        }

        public override void OnDocumentLoad(ElementDocument document) {
        }

        public override void OnDocumentUnload(ElementDocument document) {
            moonRocket.DestroyScript(document);
        }

        public override void OnElementCreate(Element element) {
            if(element.TagName.ToLower() == "script") {
                element.SetProperty("display", "none");
                moonRocket.QueueUpdate(() => {
                    moonRocket.RunScript(element.OwnerDocument, element.InnerRml);
                });
            }
        }

        public override void OnElementDestroy(Element element) {
        }
    }
}
