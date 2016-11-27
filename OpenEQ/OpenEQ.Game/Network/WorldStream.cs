using System;
using System.Text;
using static System.Console;
using static OpenEQ.Utility;

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

            var data = new byte[464];
            var str = $"{accountID}\0{sessionKey}";
            Array.Copy(Encoding.ASCII.GetBytes(str), data, str.Length);
            Send(AppPacket.Create(WorldOp.SendLoginInfo, data));
        }

        protected override void HandleAppPacket(AppPacket packet) {
            switch((WorldOp)packet.Opcode) {
                case WorldOp.GuildsList:
                    break;
                case WorldOp.LogServer:
                case WorldOp.ApproveWorld:
                case WorldOp.EnterWorld:
                case WorldOp.ExpansionInfo:
                    break;
                case WorldOp.SendCharInfo:
                    WriteLine($"HANDLED packet in WorldStream: {(WorldOp) packet.Opcode} (0x{packet.Opcode:X04})");
                    Hexdump(packet.Data);
                    var chars = new CharacterSelect(packet.Data);
                    break;
                default:
                    WriteLine($"Unhandled packet in WorldStream: {(WorldOp)packet.Opcode} (0x{packet.Opcode:X04})");
                    Hexdump(packet.Data);
                    break;
            }
        }
    }
}
