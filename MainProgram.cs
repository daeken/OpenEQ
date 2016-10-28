using System;
using static System.Console;
using OpenEQ.Engine;
using OpenEQ.GUI;
using MoonSharp.Interpreter;
using OpenTK;
using OpenEQ.Network;
using System.Collections.Generic;

namespace OpenEQ
{
    public class MainProgram
    {
        [STAThread]
        public static void Main(string[] args)
        {
            //var ls = new LoginStream("127.0.0.1", 5998);
            var ls = new LoginStream("login.eqemulator.net", 5998);
            
            var engine = new CoreEngine();
            var gui = engine.Gui;
            
            var window = gui.CreateWindow("The Zone Zone");
            var zoneinput = window.CreateTextbox(maxLength: 50);
            window.CreateButton("Load").Click += () => {
                WriteLine($"Loading zone {zoneinput.Text}");
                engine.DeleteAll();
                var zonePlaceables = OEQZoneReader.Read($"{zoneinput.Text}.zip");
                foreach(var placeable in zonePlaceables) {
                    engine.AddPlaceable(placeable);
                }
            };

            window = gui.CreateWindow("Debug");
            window.CreateLabel(() => $"Position: {engine.Camera.Position.X} {engine.Camera.Position.Y} {engine.Camera.Position.Z}");

            var loginWindow = gui.CreateWindow("Log In");
            loginWindow.CreateLabel("Username:");
            var username = loginWindow.CreateTextbox(maxLength: 16);
            loginWindow.CreateLabel("Password:");
            var password = loginWindow.CreateTextbox(maxLength: 16);
            var loggingIn = false;
            var loginButton = loginWindow.CreateButton("Log in");
            var status = loginWindow.CreateLabel("");
            loginButton.Click += () => {
                if(loggingIn)
                    return;
                WriteLine($"Attempting to log in with {username.Text}/{password.Text}");
                ls.Login(username.Text, password.Text);
                loggingIn = true;
                status.Text = "";
            };
            
            ls.LoginSuccess += (_, success) => {
                loggingIn = false;
                if(success) {
                    status.Text = "Login succeeded.  Grabbing servers.";
                    loginButton.Visible = false;
                    ls.RequestServerList();
                } else {
                    status.Text = "Login failed";
                }
            };

            var listWindow = gui.CreateWindow("Server List");
            listWindow.Visible = false;
            
            ls.ServerList += (_, list) => {
                loginWindow.Visible = false;
                foreach(var elem in list) {
                    listWindow.CreateButton(elem.longname).Click += () => {
                        WriteLine($"Trying to log into {elem.longname}");
                        ls.Play(elem);
                    };
                }
                listWindow.Visible = true;
            };

            /*var file = OEQCharReader.Read("globalpcfroglok_chr.zip");
            var charmodel = file["FRM_ACTORDEF"];
            charmodel.Animation = "L02";
            engine.AddMob(new Mob(charmodel));*/

            engine.Run();
        }
    }
}
