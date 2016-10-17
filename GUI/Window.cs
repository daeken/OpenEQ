using ImGuiNET;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenEQ.GUI {
    public class Window : IWidget {
        public string Title;
        public bool Closable = false;
        List<IWidget> widgets;
        bool open = true;
        public bool Open {
            get { return open || !Closable; }
            set { open = value; }
        }

        public Window ParentWindow {
            get { return null; }
            set { }
        }

        public Window(string title) {
            Title = title;
            widgets = new List<IWidget>();
        }
        public void Add(IWidget widget) {
            widgets.Add(widget);
            widget.ParentWindow = this;
        }
        public void Render() {
            if(Open) {
                ImGui.GetStyle().WindowRounding = 10;
                ImGui.BeginWindow(Title, ref open, WindowFlags.AlwaysAutoResize);
                ImGuiNative.igSetWindowFontScale(2.0f);

                foreach(var widget in widgets)
                    widget.Render();

                ImGui.EndWindow();
            }
        }
    }
}
