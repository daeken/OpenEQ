using OpenEQ.Engine;
using System;
using System.Collections.Generic;
using System.Linq;
using MoonSharp;
using MoonSharp.Interpreter;

namespace OpenEQ {
    public enum GameState {

    }

    [MoonSharpUserData]
    class Game {
        public CoreEngine engine;
        public Game() {
            engine = new CoreEngine();
        }

        public void Run() {
            engine.Run();
        }
    }
}
