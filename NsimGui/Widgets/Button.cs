using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using ImGuiNET;

namespace NsimGui.Widgets {
	public class Button : BaseWidget, IEnumerable<Action<Button>> {
		public Func<string> Label;
		public string LabelString {
			set => Label = () => value;
		}

		public (int W, int H) Size = (100, 40);

		public Button(string label) => LabelString = label;
		public Button(string label, (int, int) size) {
			LabelString = label;
			Size = size;
		}

		public Button(Func<string> label) => Label = label;
		public Button(Func<string> label, (int, int) size) {
			Label = label;
			Size = size;
		}

		public event Action<Button> Click;

		public Action<Button> OnClick {
			set => Click += value;
		}

		public override void Render(Gui gui) {
			if(ImGui.Button($"{Label()}###{Id}", new Vector2(Size.W, Size.H)))
				Click?.Invoke(this);
		}

		public void Add(Action<Button> onClick) => Click += onClick;
		public IEnumerator<Action<Button>> GetEnumerator() => Click?.GetInvocationList().Select(x => (Action<Button>) x).GetEnumerator();
		IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
	}
}