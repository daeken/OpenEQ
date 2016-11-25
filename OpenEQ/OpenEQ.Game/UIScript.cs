using System;
using SiliconStudio.Xenko.Engine;
using SiliconStudio.Xenko.UI;

namespace OpenEQ {
    public abstract class UIScript : SyncScript {
        protected UIComponent ui;
        protected UIPage page;

        protected T Find<T>(string name) where T : UIElement {
            return page.RootElement.FindVisualChildOfType<T>(name);
        }

        public override void Start() {
            ui = Entity.Get<UIComponent>();
            page = ui.Page;
            Setup();
        }

        public abstract void Setup();
    }
}
