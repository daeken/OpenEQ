using OpenEQ.Engine;
using System.Collections.Generic;
using MoonSharp.Interpreter;
using System.IO;
using System.Threading.Tasks;
using static System.Console;
using OpenEQ.Network;
using System;

namespace OpenEQ {
    public enum GameState {
        Login,
        World,
        Zone
    }

    [MoonSharpUserData]
    class Game {
        public static Game Instance;
        public CoreEngine Engine;
        public GameState State = GameState.Login;
        public Dictionary<string, string> Config;

        public event EventHandler<object> WorldLoginSuccess;

        LoginStream ls;
        WorldStream ws;

        public LoginStream Login {
            get {
                if(State == GameState.Login)
                    return ls;
                else
                    return null;
            }
        }

        public WorldStream World {
            get {
                if(State == GameState.World)
                    return ws;
                else
                    return null;
            }
        }

        public Game() {
            Instance = this;
            LoadConfig();
            Engine = new CoreEngine();

            var conn = Config["loginserver"].Split(':');
            ls = new LoginStream(conn[0], Int32.Parse(conn[1]));
            ls.PlaySuccess += (_, server) => {
                if(server == null)
                    WorldLoginSuccess?.Invoke(this, false);
                else {
                    ws = new WorldStream(server?.worldIP, 9000, ls.accountID, ls.sessionKey);
                    //WorldLoginSuccess?.Invoke(this, true);
                }
            };
        }

        void LoadConfig() {
            Config = new Dictionary<string, string>();
            var data = File.ReadAllText("openeq.cfg");
            foreach(var tline in data.Split('\n')) {
                var line = tline.Split(new char[] { '#' }, 2)[0].Trim();
                if(!line.Contains("="))
                    continue;
                var kv = line.Split(new char[] { '=' }, 2);
                Config[kv[0].Trim()] = kv[1].Trim();
            }
        }

        public void LoginToWorld(uint id) {
            Login.Play(id);
        }

        public void Run() {
            Engine.Run();
        }
    }
}
