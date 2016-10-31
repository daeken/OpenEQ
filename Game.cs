using OpenEQ.Engine;
using System;
using System.Collections.Generic;
using System.Linq;
using MoonSharp;
using MoonSharp.Interpreter;
using System.IO;

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

        public Game() {
            Instance = this;
            LoadConfig();
            Engine = new CoreEngine();
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

        public void Run() {
            Engine.Run();
        }
    }
}
