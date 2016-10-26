using static System.Console;

namespace OpenEQ.Network {
    public class LoginStream : EQStream {
        public LoginStream(string host, int port, string username, string password) : base(host, port) {

        }
        protected override void HandleSessionResponse(Packet packet) {
            WriteLine("Got session response in login.");
        }
    }
}
