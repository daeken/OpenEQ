using System;
using static System.Console;
using OpenEQ.Engine;
using OpenEQ.GUI;
using MoonSharp.Interpreter;
using OpenTK;

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

            window = gui.CreateWindow("Debug");
            window.CreateLabel(() => $"Position: {engine.Camera.Position.X} {engine.Camera.Position.Y} {engine.Camera.Position.Z}");

            var charmodel = OEQCharReader.Read("orc_chr.zip")["ORC_ACTORDEF"];
            charmodel.Animation = "L02";
            engine.AddMob(new Mob(charmodel));

            engine.Run();
        }
    }
}
