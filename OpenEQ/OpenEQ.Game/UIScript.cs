using System;
using SiliconStudio.Xenko.Engine;
using SiliconStudio.Xenko.UI;
using System.Linq;
using System.Reflection;
using OpenEQ.Configuration;

namespace OpenEQ {
    public abstract class UIScript : SyncScript {
        [AttributeUsage(AttributeTargets.Field, AllowMultiple = false)]
        public class PageElement : Attribute {
        }

        protected UIComponent ui;
        protected UIPage page;

        protected T Find<T>(string name) where T : UIElement {
            return page.RootElement.FindVisualChildOfType<T>(name);
        }

        void InitializeElementFields() {
            var type = GetType();
            var fields = type.GetFields(BindingFlags.DeclaredOnly | BindingFlags.NonPublic | BindingFlags.Instance);
            foreach(var field in fields) {
                if(field.CustomAttributes.ToArray().Length == 1 && field.CustomAttributes.First().AttributeType == typeof(PageElement)) {
                    var elem = page.RootElement.FindName(field.Name);
                    field.SetValue(this, elem);
                }
            }
        }

        public override void Start() {
            ui = Entity.Get<UIComponent>();
            page = ui.Page;

            InitializeElementFields();
            Setup();
        }

        public abstract void Setup();
    }
}
