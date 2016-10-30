using MoonSharp.Interpreter;
using LibRocketNet;
using static System.Console;

namespace OpenEQ.GUI.MoonRocket {
    [MoonSharpUserData]
    public static class EventGlue {
        public static Element Bind(this Element elem, string evt, DynValue callback) {
            var cb = callback.Function.GetDelegate();
            switch(evt) {
                case "show": elem.Show += (sender, e) => cb(sender, e); break;
                case "hide": elem.Hide += (sender, e) => cb(sender, e); break;
                case "resize": elem.Resize += (sender, e) => cb(sender, e); break;
                case "scroll": elem.Scroll += (sender, e) => cb(sender, e); break;
                case "focus": elem.Focus += (sender, e) => cb(sender, e); break;
                case "blur": elem.Blur += (sender, e) => cb(sender, e); break;

                case "keydown": elem.KeyDown += (sender, e) => cb(sender, e); break;
                case "keyup": elem.KeyUp += (sender, e) => cb(sender, e); break;

                case "textinput": elem.TextInput += (sender, e) => cb(sender, e); break;


                case "click": elem.Click += (sender, e) => cb(sender, e); break;
                case "dblclick": elem.DoubleClick += (sender, e) => cb(sender, e); break;
                case "mouseover": elem.MouseOver += (sender, e) => cb(sender, e); break;
                case "mouseout": elem.MouseOut += (sender, e) => cb(sender, e); break;
                case "mousemove": elem.MouseMove += (sender, e) => cb(sender, e); break;
                case "mouseup": elem.MouseUp += (sender, e) => cb(sender, e); break;
                case "mousedown": elem.MouseDown += (sender, e) => cb(sender, e); break;
                case "mousescroll": elem.MouseScroll += (sender, e) => cb(sender, e); break;

                case "dragstart": elem.DragStart += (sender, e) => cb(sender, e); break;
                case "dragend": elem.DragEnd += (sender, e) => cb(sender, e); break;
                case "drag": elem.Drag += (sender, e) => cb(sender, e); break;

                case "submit": elem.FormSubmit += (sender, e) => cb(sender, e); break;

                case "change": elem.TabChange += (sender, e) => cb(sender, e); break;

                case "load": elem.Load += (sender, e) => cb(sender, e); break;
                case "unload": elem.Unload += (sender, e) => cb(sender, e); break;

                case "handledrag": elem.HandleDrag += (sender, e) => cb(sender, e); break;

                case "columnadd": elem.ColumnAdd += (sender, e) => cb(sender, e); break;
                case "rowupdate": elem.RowUpdate += (sender, e) => cb(sender, e); break;
                case "rowadd": elem.RowAdd += (sender, e) => cb(sender, e); break;
                case "rowremove": elem.RowRemove += (sender, e) => cb(sender, e); break;

                case "tabchange": elem.TabChange += (sender, e) => cb(sender, e); break;
                default:
                    WriteLine($"Unknown event in bind: {evt}");
                    break;
            }
            return elem;
        }
    }
}
