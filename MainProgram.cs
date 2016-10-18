using System;
using static System.Console;
using OpenEQ.Engine;
using OpenEQ.GUI;
using MoonSharp.Interpreter;

namespace OpenEQ
{
    public class MainProgram
    {
        [STAThread]
        public static void Main(string[] args)
        {
            var engine = new CoreEngine();
            var gui = engine.Gui;
            /*var code = @"
                function testfunc()
                    print(""Test"")
                    print(textbox.Text)
                end
                win = gui.CreateWindow('Test')
                textbox = win.CreateTextbox()
                button = win.CreateButton('Test button')
                button.Click.add(testfunc)
            ";
            UserData.RegisterAssembly();
            var script = new Script();
            script.Globals["gui"] = gui;
            script.DoString(code);*/

            var window = gui.CreateWindow("The Zone Zone");
            var zoneinput = window.CreateTextbox(maxLength: 50);
            var button = window.CreateButton("Load");
            button.Click += () => {
                WriteLine($"Loading zone {zoneinput.Text}");
                engine.DeleteAll();
                var zonePlaceables = OEQZoneReader.Read($"{zoneinput.Text}.zip");
                foreach(var placeable in zonePlaceables) {
                    engine.AddPlaceable(placeable);
                }
            };

            window = gui.CreateWindow("Model Loader");
            var modelinput = window.CreateTextbox(maxLength: 50);
            button = window.CreateButton("Load");
            button.Click += () => {
                WriteLine($"Loading model file {modelinput.Text}");
                var mf = OEQCharReader.Read($"{modelinput.Text}.zip");
                foreach(var mn in mf.Keys)
                    WriteLine(mn);
            };

            engine.Run();
        }
    }
}
