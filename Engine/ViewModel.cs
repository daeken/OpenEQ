using System;
using System.Collections.Generic;
using IronPython.Runtime;
using MoreLinq;
using Noesis;
using PrettyPrinter;

namespace OpenEQ.Engine {
	public class ViewModel : Dictionary<string, object> {
		readonly FrameworkElement BoundTo;
		readonly Dictionary<string, Func<object>> Reactives = new Dictionary<string, Func<object>>();
		
		public ViewModel(FrameworkElement to) {
			BoundTo = to;
			to.DataContext = this;
		}

		public void Update() {
			var changed = false;
			foreach(var (k, v) in Reactives) {
				var orig = ((Dictionary<string, object>) this)[k];
				var cur = ((Dictionary<string, object>) this)[k] = v();
				if(orig != cur) changed = true;
			}

			if(changed) {
				BoundTo.DataContext = null;
				BoundTo.DataContext = this;
			}
		}

		public new object this[string key] {
			get => ContainsKey(key) ? ((Dictionary<string, object>) this)[key] : BoundTo.FindName(key);
			set {
				switch(value) {
					case PythonFunction pf:
						this[key] = (Func<object>) (dynamic) pf;
						return;
					case Func<object> reactive: {
						if(ContainsKey(key)) Remove(key);
						Reactives[key] = reactive;
						((Dictionary<string, object>) this)[key] = reactive();
						return;
					}
				}

				if(Reactives.ContainsKey(key)) Reactives.Remove(key);
				if(ContainsKey(key) && ((Dictionary<string, object>) this)[key] == value) return;
				BoundTo.DataContext = null;
				((Dictionary<string, object>) this)[key] = value;
				BoundTo.DataContext = this;
			}
		}
	}
}