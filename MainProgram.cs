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
            //var ls = new LoginStream("127.0.0.1", 5998);
            //var ls = new LoginStream("login.eqemulator.net", 5998);

            var game = new Game();
            game.Run();
        }
    }
}
