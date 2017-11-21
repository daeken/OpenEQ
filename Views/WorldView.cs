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
	class WorldView : Node {
		WorldController Controller = WorldController.Instance;

		[ControlGetter] ItemList CharacterList = null;

		public override void _Ready() {
			Controller.Register(this);
			Controller.Connect();
			CharacterList.Connect("item_activated", (int idx) => {
				if(idx < 1)
					return;
				Controller.SelectCharacter(idx - 1);
			});
		}

		public void ShowCharacters(List<CharacterSelectEntry> chars) {
			while(CharacterList.GetItemCount() > 1)
				CharacterList.RemoveItem(1);
			foreach(var item in chars)
				CharacterList.AddItem($"Character {item.Name} - Level {item.Level}", selectable: false);
		}

		public void LoadZoneScene() {
			GetTree().ChangeScene("res://Zone.tscn");
		}
	}
}
