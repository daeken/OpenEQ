using ImGuiNET;
using MoonSharp.Interpreter;
using System;
using System.Runtime.InteropServices;
using System.Text;
using static System.Console;

namespace OpenEQ.GUI {
    [MoonSharpUserData]
    public unsafe class Textbox : BaseWidget {
        public string Label;

        int maxLength;
        byte* textBuffer;

        unsafe public string Text {
            get {
                var ptr = new IntPtr(textBuffer);
                var len = 0;
                while(Marshal.ReadByte(ptr, len) != 0) ++len;
                var buffer = new byte[len];
                Marshal.Copy(ptr, buffer, 0, buffer.Length);
                return Encoding.UTF8.GetString(buffer);
            }
        }

        public Textbox(string label = "", int maxLength = 256) {
            Label = label;
            this.maxLength = maxLength;
            textBuffer = (byte *) Marshal.AllocHGlobal(maxLength * 4); // UTF-8 -- could be up to 4 bpc
            textBuffer[0] = 0;
        }

        public override void Render() {
            if(Visible)
                ImGui.InputText(Label, new IntPtr(textBuffer), (uint) maxLength * sizeof(long), InputTextFlags.Default, null);
        }
    }
}
