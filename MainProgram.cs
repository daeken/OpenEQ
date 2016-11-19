using System;
using static System.Console;
using OpenEQ.Engine;
using OpenEQ.Network;

namespace OpenEQ
{
    public class MainProgram
    {
        [STAThread]
        public static void Main(string[] args)
        {
            var game = new Game();
            game.Run();
        }
    }
}
