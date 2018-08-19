using NsimGui;
using NsimGui.Widgets;
using OpenEQ.Engine;

namespace OpenEQ.Views {
	public class ModelMeshView : BaseView {
		public ModelMeshView(Controller controller) : base(controller) {}

		public override void Setup(Gui gui) {
			var window = new Window("Model Meshes") {
				new Size(400, 400),
				new Button(() => Globals.Stopwatch.IsRunning ? "Pause" : "Unpause") { _ => {
						if(Globals.Stopwatch.IsRunning)
							Globals.Stopwatch.Stop();
						else
							Globals.Stopwatch.Start();
					}
				}
			};
			Controller.LastModelLoaded.Meshes.ForEach(mesh =>
				window.Add(new Checkbox(mesh.Material.ToString(), true) { x => mesh.Enabled = x.Checked }));
			gui.Add(window);
		}
	}
}