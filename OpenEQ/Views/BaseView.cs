using NsimGui;

namespace OpenEQ.Views {
	public abstract class BaseView {
		public readonly Controller Controller;
		protected BaseView(Controller controller) => Controller = controller;
		
		public virtual void Setup(Gui gui) {}
		public virtual void Update(Gui gui) {}
		public virtual void Teardown(Gui gui) {}
	}
}