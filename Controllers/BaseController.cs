using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using Godot;
using OpenEQ.Network;

namespace OpenEQ.Controllers {
	abstract class BaseController<StreamT, ViewT> where StreamT : EQStream where ViewT : class {
		WeakReference<ViewT> ViewRef = new WeakReference<ViewT>(null);
		protected ViewT View {
			get { return ViewRef.Get(); }
			set { ViewRef.SetTarget(value); }
		}

		StreamT _Connection;
		internal StreamT Connection {
			get {
				if(_Connection == null)
					_Connection = InitializeConnection();
				return _Connection;
			}
		}

		protected abstract StreamT InitializeConnection();

		public void Register(ViewT view) {
			View = view;
		}
		public void Unregister(ViewT view) {
			if(view == View)
				View = null;
		}

		protected void RequireView() {
			if(View == null)
				throw new Exception($"View not assigned in controller {this}");
		}

		public StreamT Connect() {
			return Connection;
		}
	}
}
