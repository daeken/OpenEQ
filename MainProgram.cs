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
            var zoneObjects = OEQZoneReader.Read("gfaydark.zip");
            foreach(var obj in zoneObjects) {
                engine.AddObject(obj);
            }
            engine.Run();
        }
    }
}
