using System;
using System.Collections.Generic;
using System.Text;
using static System.Console;
using static OpenEQ.Network.Utility;

namespace OpenEQ.Network {
    public class ZoneStream : EQStream {
        string charName;

        public ZoneStream(string host, int port, string charName) : base(host, port) {
            this.charName = charName;

            WriteLine("Starting zone connection...");
            Connect();
            SendSessionRequest();
        }

        protected override void HandleSessionResponse(Packet packet) {
            Send(packet);

            Send(AppPacket.Create(ZoneOp.ZoneEntry, new ClientZoneEntry(charName)));
        }

        protected override void HandleAppPacket(AppPacket packet) {
            switch((ZoneOp) packet.Opcode) {
                case ZoneOp.PlayerProfile:
                    var player = packet.Get<PlayerProfile>();
                    WriteLine(player);
                    break;

                default:
                    WriteLine($"Unhandled packet in ZoneStream: {(ZoneOp) packet.Opcode} (0x{packet.Opcode:X04})");
                    Hexdump(packet.Data);
                    break;
            }
        }
    }
}
