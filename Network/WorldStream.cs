using System;
using System.Text;
using static System.Console;

namespace OpenEQ.Network {
    public class WorldStream : EQStream {
        uint accountID;
        string sessionKey;

        public WorldStream(string host, int port, uint accountID, string sessionKey) : base(host, port) {
            this.accountID = accountID;
            this.sessionKey = sessionKey;
            WriteLine("Starting world connection...");
            Connect();
            SendSessionRequest();
        }

        protected override void HandleSessionResponse(Packet packet) {
            Send(packet);

            var data = new byte[488];
            var str = $"{accountID}\0{sessionKey}";
            Array.Copy(Encoding.ASCII.GetBytes(str), data, str.Length);
            Send(AppPacket.Create(WorldOp.SendLoginInfo, data));
        }

        protected override void HandleAppPacket(AppPacket packet) {
            throw new NotImplementedException();
        }
    }
}
