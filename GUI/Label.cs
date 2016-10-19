using ImGuiNET;
using MoonSharp.Interpreter;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenEQ.GUI {
    [MoonSharpUserData]
    public class Label : BaseWidget {
        public string Text;
        public Func<string> Update;
        public event Action Click;

        public Label(string text) {
            Text = text;
            Update = null;
        }

        public Label(Func<string> update) {
            Text = null;
            Update = update;
        }

        public override void Render() {
            if(Visible) {
                if(Text == null)
                    ImGui.Text(Update());
                else
                    ImGui.Text(Text);
            }
        }
    }
}
