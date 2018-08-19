using System;
using System.Numerics;
using MoreLinq;
using ImGuiNET;

namespace NsimGui.Widgets {
	public class HBox : BaseContainerWidget {
		public override void Render(Gui gui) {
			var last = Children.Count - 1;
			Children.ForEach((child, i) => {
				child.Render(gui);
				if(i != last)
					ImGui.SameLine();
			});
		}
	}
}