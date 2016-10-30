using LibRocketNet;
using MoonSharp;
using MoonSharp.Interpreter;
using System;
using System.Collections.Generic;
using System.Reflection;
using System.Text.RegularExpressions;
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

            UserData.RegisterAssembly(includeExtensionTypes: true);
            RegisterUnmarkedAssembly(typeof(Element));
        }

        public void RegisterUnmarkedAssembly(Type _type = null) {
            var assembly = _type != null ? _type.Assembly : Assembly.GetCallingAssembly();

            foreach(var type in assembly.DefinedTypes) {
                if(type.IsPublic)
                    UserData.RegisterType(type);
            }
        }

        DynValue FindValue(Script ctx, string code) {
            DynValue cur = null;
            foreach(var part in code.Split(':', '.')) {
                if(cur == null)
                    cur = ctx.Globals.Get(part);
                else
                    cur = cur.Table.Get(part);
                if(cur == null) {
                    WriteLine("Could not find part {part} in event reference {code}");
                    return null;
                }
            }
            return cur;
        }

        public void ProcessScriptEvent(Element self, string code, ElementEventArgs e, bool isBare = false) {
            var ctx = GetContext(self.OwnerDocument);

            if(isBare) {
                var func = FindValue(ctx, code);
                if(func == null)
                    return;
                func.Function.GetDelegate()(self, e);
            } else {
                var oldSelf = ctx.Globals["self"];
                var oldE = ctx.Globals["e"];
                ctx.Globals["self"] = self;
                ctx.Globals["e"] = e;
                ctx.DoString(code);
                ctx.Globals["self"] = oldSelf;
                ctx.Globals["e"] = oldE;
            }
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
