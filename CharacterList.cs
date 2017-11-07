using Godot;
using OpenEQ;
using OpenEQ.Network;
using System;
using System.Collections.Generic;

public class CharacterList : ItemList {
	List<CharacterSelectEntry> charList;

    public override void _Ready() {
		Connect("item_activated", this, "Clicked");

		LogicBridge.Instance.OnCharacterList += (_, chars) => {
			charList = chars;
			foreach(var c in chars)
				AddItem($"Character {c.Name} - Level {c.Level}", selectable: false);
		};
		LogicBridge.Instance.ConnectToWorld();
	}

	void Clicked(int index) {
		if(index == 0)
			return;

		LogicBridge.Instance.EnterWorld(charList[index - 1]);
		LogicBridge.Instance.OnCharacterSpawn += (_, __) => GetTree().ChangeScene("res://Zone.tscn");
	}
}
