using OpenEQ.Network;
using System.Threading;
using static System.Console;

namespace OpenEQ.NetClient {
    class Program {
        static void Main(string[] args) {
            var running = true;
            var login = new LoginStream("192.168.1.119", 5998);

            login.LoginSuccess += (sender, success) => {
                if(success) {
                    WriteLine("Login successful. Requesting server list.");
                    login.RequestServerList();
                } else {
                    WriteLine("Login failed.");
                    running = false;
                }
            };

            login.ServerList += (sender, servers) => {

            };

            login.PlaySuccess += (sender, server) => {

            };

            login.Login("testacct", "testacct");

            while(running)
                Thread.Sleep(100);

            WriteLine("Completed.");
            ReadLine();
        }
    }
}
