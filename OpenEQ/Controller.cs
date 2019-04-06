using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using IronPython.Hosting;
using Noesis;
using OpenEQ.Engine;
using PrettyPrinter;
using static OpenEQ.Engine.Globals;

namespace OpenEQ {
	public class Controller {
		public static readonly Controller Instance = new Controller();
		public readonly EngineCore Engine = new EngineCore();
		
		public AniModel LastModelLoaded;
		
		public readonly List<WeakReference<ViewModel>> ViewModels = new List<WeakReference<ViewModel>>();

		public void LoadZone(string name) {
			// TODO: Unload old contents
			Loader.LoadZoneFile($"../ConverterApp/{name}_oes.zip", Engine);
		}

		public void LoadCharacter(string filename, string name) {
			var model = LastModelLoaded = Loader.LoadCharacter($"../ConverterApp/{filename}_oes.zip", name);
			var instance = new AniModelInstance(model) { Animation = "C05", Position = vec3(-153, 149, 80) };
			Engine.Add(instance);
		}

		public void Start() {
			Engine.UpdateFrame += (_, __) => ViewModels.ForEach(modelref => {
				if(modelref.TryGetTarget(out var model))
					model.Update();
			});
			
			foreach(var fn in Directory.EnumerateFiles("UI", "*.py").Concat(Directory.EnumerateFiles("UI", "*.xaml"))) {
				var engine = Python.CreateEngine();
				var scope = engine.CreateScope();
				engine.CreateScriptSourceFromString(@"
import clr
clr.AddReference('Noesis.App')
clr.AddReference('Noesis.GUI')
clr.AddReference('OpenEQ')
clr.AddReference('OpenEQ.Engine')
clr.AddReference('OpenEQ.Common')
from System.MathF import *
from OpenEQ import *
from OpenEQ.Engine import *
from OpenEQ.Common import *
controller = Controller.Instance
engine = controller.Engine

def bind(target):
    def sub(func):
        to = target
        if func.__code__.co_argcount == 0:
            to += lambda _, __: func()
        elif func.__code__.co_argcount == 1:
            to += lambda _, b: func(b)
        else:
            to += func
    return sub

import re

def loadXaml(xaml):
    bindings = {}
    def repl(mo):
        code = mo.group(1)
        bindings['_%i' % len(bindings)] = eval('lambda: ' + code)
        return '{Binding [_%i]}' % (len(bindings) - 1)
    xaml = re.sub(r'\{`([^`]+)`\}', repl, xaml)
    model = controller.LoadXaml(xaml)
    for k, v in bindings.items():
        model[k] = v
    return model
				").Execute(scope);
				var source = fn.EndsWith(".xaml")
					? engine.CreateScriptSourceFromString($"loadXaml('''{File.ReadAllText(fn)}''')")
					: engine.CreateScriptSourceFromFile(fn);
				source.Execute(scope);
			}
			Engine.Start();
		}

		public ViewModel LoadXaml(string xaml) {
			var xo = (FrameworkElement) GUI.ParseXaml(xaml);
			((Grid) Engine.View.Content).Children.Add(xo);
			var vm = new ViewModel(xo);
			ViewModels.Add(new WeakReference<ViewModel>(vm));
			return vm;
		}
	}
}