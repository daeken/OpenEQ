using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using ImGuiNET;

namespace NsimGui.Widgets {
	public class Checkbox : BaseWidget, IEnumerable<Action<Checkbox>> {
		public Func<string> Label;
		public bool Checked;
		public string LabelString {
			set => Label = () => value;
		}

		public Checkbox(string label, bool @checked = false) {
			LabelString = label;
			Checked = @checked;
		}

		public Checkbox(Func<string> label, bool @checked = false) {
			Label = label;
			Checked = @checked;
		}

		public event Action<Checkbox> Change;

		public Action<Checkbox> OnChange {
			set => Change += value;
		}

		public override void Render(Gui gui) {
			var prev = Checked;
			ImGui.Checkbox($"{Label()}###{Id}", ref Checked);
			if(prev != Checked)
				Change?.Invoke(this);
		}

		public void Add(Action<Checkbox> onChange) => Change += onChange;
		public IEnumerator<Action<Checkbox>> GetEnumerator() => Change?.GetInvocationList().Select(x => (Action<Checkbox>) x).GetEnumerator();
		IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
	}
}