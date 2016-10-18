using ImGuiNET;
using MoonSharp.Interpreter;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenEQ.GUI {
    [MoonSharpUserData]
    public class Button : IWidget {
        public string Label;
        public event Action Click;
        public Window ParentWindow { get; set; }

        public Button(string label) {
            Label = label;
        }

        public void Render() {
            if(ImGui.Button(Label) && Click != null) {
                Click();
            }
        }
    }
}
