using LibRocketNet;
using MoonSharp;
using MoonSharp.Interpreter;
using System;
using System.Collections.Generic;
using static System.Console;

namespace OpenEQ.GUI.MoonRocket {
    public class CoreMoonRocket {
        public CoreGUI Gui;
        MoonRocketPlugin plugin;
        MoonListenerInstancer instancer;
        Queue<Action> updateQueue = new Queue<Action>();
        Dictionary<ElementDocument, Script> contexts = new Dictionary<ElementDocument, Script>();
        public CoreMoonRocket(CoreGUI gui) {
            plugin = new MoonRocketPlugin(this);
            plugin.Register();
            instancer = new MoonListenerInstancer(this);
            instancer.Register();

            UserData.RegisterType<ElementDocument>();
            UserData.RegisterType<Element>();
            UserData.RegisterType<ElementEventArgs>();
            UserData.RegisterType<Color>();
            UserData.RegisterType<CoreGUI>();
        }

        public void ProcessScriptEvent(Element self, string code, ElementEventArgs e) {
            var ctx = GetContext(self.OwnerDocument);
            var oldSelf = ctx.Globals["self"];
            var oldE = ctx.Globals["e"];
            ctx.Globals["self"] = self;
            ctx.Globals["e"] = e;
            ctx.DoString(code);
            ctx.Globals["self"] = oldSelf;
            ctx.Globals["e"] = oldE;
        }

        public void RunScript(ElementDocument document, string code) {
            var ctx = GetContext(document);
            ctx.DoString(code);
        }

        Script GetContext(ElementDocument document) {
            if(contexts.ContainsKey(document))
                return contexts[document];

            var script = contexts[document] = new Script();
            script.Globals["document"] = document;
            script.Globals["gui"] = Gui;
            return script;
        }

        public void DestroyScript(ElementDocument document) {
            if(contexts.ContainsKey(document))
                contexts.Remove(document);
        }

        public void Update() {
            while(updateQueue.Count > 0)
                updateQueue.Dequeue()();
        }

        public void QueueUpdate(Action func) {
            updateQueue.Enqueue(func);
        }
    }
}
