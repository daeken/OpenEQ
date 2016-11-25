using System;
using System.Linq;
using SiliconStudio.Xenko.UI.Controls;

namespace OpenEQ
{
    public class LoginScript : UIScript
    {
        public string LoginServer = "login.eqemulator.net:5998";

        public override void Setup() {
            var loginButton = Find<Button>("loginButton");
            var username = Find<EditText>("username");
            var password = Find<EditText>("password");
            var status = Find<TextBlock>("status");
            loginButton.Click += (sender, e) => {
                loginButton.IsEnabled = false;
                username.Text = String.Join("", username.Text.Reverse().ToArray());
            };
        }

        public override void Update() {
        }
    }
}
