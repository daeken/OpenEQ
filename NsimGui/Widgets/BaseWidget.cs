using System.Collections;
using System.Collections.Generic;

namespace NsimGui.Widgets {
	public abstract class BaseWidget {
		public ulong Id = Gui.UniqueID;

		public abstract void Render(Gui gui);
	}

	public abstract class BaseContainerWidget : BaseWidget, IEnumerable<BaseWidget> {
		public readonly List<BaseWidget> Children = new List<BaseWidget>();

		public void Add(BaseWidget widget) => Children.Add(widget);

		public IEnumerator<BaseWidget> GetEnumerator() => Children.GetEnumerator();
		IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
	}
}