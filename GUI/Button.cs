using ImGuiNET;
using MoonSharp.Interpreter;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenEQ.GUI {
    [MoonSharpUserData]
    public class Button : BaseWidget {
        public string Label;
        public event Action Click;

        public Button(string label) {
            Label = label;
        }

        public override void Render() {
            if(Visible)
                if(ImGui.Button(Label) && Click != null)
                    Click();
        }
    }
}
