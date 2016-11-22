using SiliconStudio.Xenko.Engine;

namespace OpenEQ
{
    class OpenEQApp
    {
        static void Main(string[] args)
        {
            using (var game = new Game())
            {
                game.Run();
            }
        }
    }
}
