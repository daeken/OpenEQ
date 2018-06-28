using System;
using System.Numerics;
using ImGuiNET;

namespace NsimGui.Widgets {
	public class Window : BaseContainerWidget {
		public Func<string> Title;
		public string TitleString {
			set => Title = () => value;
		}

		(int W, int H) _Size = (400, 400);
		public (int W, int H) Size {
			get => _Size;
			set {
				SetSize = true;
				_Size = value;
			}
		}
		bool SetSize = true;

		(int X, int Y) _Position = (-1, -1);
		public (int X, int Y) Position {
			get => _Position;
			set {
				SetPosition = false;
				_Position = value;
			}
		}
		bool SetPosition = true;

		public Window(string title) => TitleString = title;
		public Window(Func<string> title) => Title = title;
		
		static int TWidth = 100;

		bool HasRendered;
		
		public override void Render(Gui gui) {
			if(SetSize) {
				ImGui.SetNextWindowSize(new Vector2(Size.W, Size.H), Condition.Always);
				SetSize = false;
			}

			if(SetPosition) {
				if(Position.X == -1 && Position.Y == -1)
					Position = (TWidth, 200);
				ImGui.SetNextWindowPos(new Vector2(Position.X, Position.Y), Condition.Always, Vector2.Zero);
				SetPosition = false;
			}

			if(!HasRendered) {
				HasRendered = true;
				TWidth += Size.W + 50;
			}
			
			ImGui.BeginWindow($"{Title()}###{Id}");
			foreach(var child in Children)
				child.Render(gui);
			ImGui.EndWindow();
		}
	}
}