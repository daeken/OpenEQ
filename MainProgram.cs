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
            var ls = new LoginStream("login.eqemulator.net", 5998);
            
            var engine = new CoreEngine();

            var file = OEQCharReader.Read("globalpcfroglok_chr.zip");
            var charmodel = file["FRM_ACTORDEF"];
            charmodel.Animation = "L02";
            engine.AddMob(new Mob(charmodel));

            engine.Run();
        }
    }
}
