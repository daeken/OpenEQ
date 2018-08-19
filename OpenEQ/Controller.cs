using System.Collections.Generic;
using OpenEQ.Engine;
using OpenEQ.Views;
using static OpenEQ.Engine.Globals;

namespace OpenEQ {
	public class Controller {
		public readonly EngineCore Engine = new EngineCore();
		
		readonly List<BaseView> Views = new List<BaseView>();

		public AniModel LastModelLoaded;

		public Controller() =>
			Engine.UpdateFrame += (s, e) => Views.ForEach(view => view.Update(Engine.Gui));

		public void AddView(BaseView view) {
			Views.Add(view);
			view.Setup(Engine.Gui);
		}

		public void RemoveView(BaseView view) {
			Views.Add(view);
			view.Teardown(Engine.Gui);
		}

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
			Engine.Run();
		}
	}
}