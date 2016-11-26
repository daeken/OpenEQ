using OpenEQ.Network;
using System.Threading;
using static System.Console;

namespace OpenEQ.NetClient {
    class Program {
        static void Main(string[] args) {
            EQStream.Debug = true;

            var running = true;
            var login = new LoginStream("192.168.1.119", 5998);
            WorldStream world;

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
                WriteLine($"Got {servers.Count} servers:");
                foreach(var server in servers) {
                    WriteLine($"- '{server.longname}' @ {server.worldIP} is {server.status} with {server.playersOnline} players");
                }
                var chosen = servers[0];
                WriteLine($"Sending play request for server '{chosen.longname}' @ {chosen.worldIP}");
                login.Play(chosen);
            };

            login.PlaySuccess += (sender, server) => {
                if(server == null) {
                    WriteLine("Play request rejected.");
                    running = false;
                } else {
                    WriteLine("Play request accepted.  Initializing world.");
                    world = new WorldStream(server?.worldIP, 9000, login.accountID, login.sessionKey);
                }
            };

            login.Login("testacct", "testacct");

            while(running)
                Thread.Sleep(100);

            WriteLine("Completed.");
            ReadLine();
        }
    }
}
