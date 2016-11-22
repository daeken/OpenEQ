using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using SiliconStudio.Core.Mathematics;
using SiliconStudio.Xenko.Input;
using SiliconStudio.Xenko.Engine;
using SiliconStudio.Xenko.UI.Controls;
using SiliconStudio.Xenko.UI.Panels;
using SiliconStudio.Xenko.UI;

namespace OpenEQ
{
    public class LoginScript : SyncScript
    {
        public string LoginServer = "login.eqemulator.net:5998";

        UIComponent ui;
        UIPage page;

        T Find<T>(string name) where T : UIElement {
            return page.RootElement.FindVisualChildOfType<T>(name);
        }

        public override void Start() {
            ui = Entity.Get<UIComponent>();
            page = ui.Page;
            var loginButton = Find<Button>("loginButton");
            var username = Find<EditText>("username");
            loginButton.Click += (sender, e) => {
                loginButton.IsEnabled = false;
                username.Text = String.Join("", username.Text.Reverse().ToArray());
            };
        }

        public override void Update() {
        }
    }
}
