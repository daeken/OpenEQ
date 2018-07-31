using NsimGui;
using NsimGui.Widgets;
using OpenEQ.Engine;

namespace OpenEQ.Views {
	public class StatusView : BaseView {
		public StatusView(Controller controller) : base(controller) {}

		public override void Setup(Gui gui) {
			gui.Add(new Window("Status") {
				new Size(500, 100),
				new Text(() => $"Position {Globals.Camera.Position}"),
				new Text(() => $"FPS {Controller.Engine.FPS}")
			});
		}
	}
}