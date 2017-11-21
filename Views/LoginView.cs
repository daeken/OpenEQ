using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using static System.Console;
using Godot;
using OpenEQ.Controllers;
using OpenEQ.Network;

namespace OpenEQ.Views {
	class LoginView : Node {
		LoginController Controller = LoginController.Instance;

		[ControlGetter("LoginScreen")] Control LoginScreen = null;
		[ControlGetter("LoginScreen/Username")] TextEdit UsernameField = null;
		public string Username {
			get { return UsernameField.GetText(); }
			set { UsernameField.SetText(value); }
		}
		[ControlGetter("LoginScreen/Password")] TextEdit PasswordField = null;
		public string Password {
			get { return PasswordField.GetText(); }
			set { PasswordField.SetText(value); }
		}
		[ControlGetter("LoginScreen/Login")] Button LoginButton = null;
		[ControlGetter("LoginScreen/LoginError")] Label LoginErrorLabel = null;
		public string LoginError {
			set => LoginErrorLabel.SetText(value);
		}

		[ControlGetter] ItemList ServerList = null;

		public override void _Ready() {
			Controller.Register(this);
			LoginButton.Connect("pressed", Controller.StartLogin);

			UsernameField.SetText("daeken");
			PasswordField.SetText("omgwtfbbq");

			ServerList.Connect("item_activated", (int idx) => {
				if(idx < 3)
					return;
				Controller.SelectServer(idx / 3 - 1);
			});
		}

		public void ShowServers(List<ServerListElement> servers) {
			while(ServerList.GetItemCount() > 3)
				ServerList.RemoveItem(3);
			foreach(var item in servers) {
				ServerList.AddItem(item.Longname, selectable: false);
				ServerList.AddItem(item.WorldIP, selectable: false);
				ServerList.AddItem($"Connect to world {item.ServerListID}", selectable: false);
			}

			LoginScreen.Hide();
			ServerList.Show();
		}

		public void LoadWorldScene() {
			GetTree().ChangeScene("res://World.tscn");
		}
	}
}
