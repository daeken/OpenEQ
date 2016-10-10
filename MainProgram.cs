using System;
using static System.Console;
using OpenEQ.Engine;

namespace OpenEQ
{
    public class MainProgram
    {
        [STAThread]
        public static void Main(string[] args)
        {
            var engine = new CoreEngine();
            var zonePlaceables = OEQZoneReader.Read("gfaydark.zip");
            foreach(var placeable in zonePlaceables) {
                engine.AddPlaceable(placeable);
            }
            engine.Run();
        }
    }
}
