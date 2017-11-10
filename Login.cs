using Godot;
using OpenEQ;
using System;
using static System.Console;

public class Login : Button
{
	private string username;
	private string password;
	private string loginServer;
	private ConfigFile config;

	public void LoadConfig()
	{
		config = new ConfigFile();
		var err = config.Load("user://settings.cfg");
		if (err == GD.OK) {
			WriteLine("Loaded config");
			username = (string) config.GetValue("login_settings", "username", "");
			password = (string) config.GetValue("login_settings", "password", "");
			loginServer = (string) config.GetValue("login_settings", "login_server", "");
		}
		else {
			WriteLine("NOT Loaded config");
			username = "";
			password = "";
			loginServer = "";
			SaveConfig();
		}
	}

	public void SaveConfig()
	{
		var tusername = ((LineEdit) GetNode("../Username")).GetText();
		var tpassword = ((LineEdit) GetNode("../Password")).GetText();
		var tloginServer = ((LineEdit) GetNode("../LoginServer")).GetText();
		config.SetValue("login_settings", "username", tusername);
		config.SetValue("login_settings", "password", tpassword);
		config.SetValue("login_settings", "login_server", tloginServer);
		config.Save("user://settings.cfg");
	}

	public override void _Ready()
	{
		LoadConfig();
		Connect("pressed", this, "Test");
		((LineEdit) GetNode("../Username")).SetText(username);
		((LineEdit) GetNode("../Password")).SetText(password);
		((LineEdit) GetNode("../LoginServer")).SetText(loginServer);

		LogicBridge.Instance.OnLoginResult += (_, success) => {
			if(success)
				GetTree().ChangeScene("res://ServerSelect.tscn");
			else
				((RichTextLabel) GetNode("../ErrorLabel")).SetText("Login failed");
		};
	}

	public void Test() {
		SaveConfig();
		LogicBridge.Instance.Login(username, password, loginServer);
	}
}
