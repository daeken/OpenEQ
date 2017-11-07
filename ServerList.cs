using Godot;
using OpenEQ;
using OpenEQ.Network;
using System.Collections.Generic;
using static System.Console;

public class ServerList : ItemList {
	List<ServerListElement> serverList;

    public override void _Ready() {
		Connect("item_activated", this, "Clicked");
		LogicBridge.Instance.OnServerList += (_, servers) => {
			serverList = servers;
			foreach(var elem in servers) {
				AddItem(elem.Longname, selectable: false);
				AddItem(elem.WorldIP, selectable: false);
				AddItem($"Connect to world {elem.ServerListID}", selectable: false);
			}
		};
		LogicBridge.Instance.RequestServerList();

		LogicBridge.Instance.OnPlayResult += (_, success) => {
			if(success)
				GetTree().ChangeScene("res://CharacterSelect.tscn");
		};
    }

	void Clicked(int index) {
		if(index < 3)
			return;
		var server = serverList[(index - 3) / 3];
		LogicBridge.Instance.Play(server);
	}
}
