using System;

namespace OpenEQ.Engine {
	public class LazyProperty<T> {
		readonly Func<T> Func;
		bool Resolved;
		T _Value;

		public T Value {
			get {
				if(!Resolved) {
					_Value = Func();
					Resolved = true;
				}
				return _Value;
			}
		}

		public LazyProperty(Func<T> func) => Func = func;

		public static implicit operator T(LazyProperty<T> lp) => lp.Value;
	}

}