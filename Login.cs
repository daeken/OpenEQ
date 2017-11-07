using Godot;
using OpenEQ;
using System;
using static System.Console;

public class Login : Button
{
    public override void _Ready()
    {
		Connect("pressed", this, "Test");
		((TextEdit) GetNode("../Username")).SetText("daeken");
		((TextEdit) GetNode("../Password")).SetText("omgwtfbbq");

		LogicBridge.Instance.OnLoginResult += (_, success) => {
			if(success)
				GetTree().ChangeScene("res://ServerSelect.tscn");
			else
				((RichTextLabel) GetNode("../ErrorLabel")).SetText("Login failed");
		};
	}

	public void Test() {
		var username = ((TextEdit) GetNode("../Username")).GetText();
		var password = ((TextEdit) GetNode("../Password")).GetText();

		LogicBridge.Instance.Login(username, password);
	}
}
