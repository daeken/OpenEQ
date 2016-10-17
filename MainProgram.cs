using System;
using static System.Console;
using OpenEQ.Engine;
using OpenEQ.GUI;

namespace OpenEQ
{
    public class MainProgram
    {
        [STAThread]
        public static void Main(string[] args)
        {
            var engine = new CoreEngine();
            var gui = engine.Gui;
            var window = new Window("The Zone Zone");
            var zoneinput = new Textbox(maxLength: 50);
            window.Add(zoneinput);
            var button = new Button("Load");
            button.Click += (sender, win) => {
                WriteLine($"Loading zone {zoneinput.Text}");
                engine.DeleteAll();
                var zonePlaceables = OEQZoneReader.Read($"{zoneinput.Text}.zip");
                foreach(var placeable in zonePlaceables) {
                    engine.AddPlaceable(placeable);
                }
            };
            window.Add(button);
            gui.Add(window);
            engine.Run();
        }
    }
}
