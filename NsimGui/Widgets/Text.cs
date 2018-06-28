using System;
using ImGuiNET;

namespace NsimGui.Widgets {
	public class Text : BaseWidget {
		public Func<string> Label;
		public string LabelString {
			set => Label = () => value;
		}

		public Text(string label) => LabelString = label;
		public Text(Func<string> label) => Label = label;

		public override void Render(Gui gui) => ImGui.Text(Label());
	}
}