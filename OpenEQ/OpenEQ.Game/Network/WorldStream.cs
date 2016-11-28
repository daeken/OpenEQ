using System;
using System.Collections.Generic;
using System.Text;
using static System.Console;
using static OpenEQ.Utility;

namespace OpenEQ.Network {
    public class WorldStream : EQStream {
        public event EventHandler<List<CharacterSelectEntry>> CharacterList;
        public event EventHandler<string> MOTD;
        public event EventHandler<ZoneServerInfo> ZoneServer;

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
                    var chars = new CharacterSelect(packet.Data);
                    CharacterList?.Invoke(this, chars.Characters);
                    break;
                case WorldOp.MessageOfTheDay:
                    MOTD?.Invoke(this, Encoding.ASCII.GetString(packet.Data));
                    break;
                case WorldOp.ZoneServerInfo:
                    var info = packet.Get<ZoneServerInfo>();
                    ZoneServer?.Invoke(this, info);
                    break;
                default:
                    WriteLine($"Unhandled packet in WorldStream: {(WorldOp)packet.Opcode} (0x{packet.Opcode:X04})");
                    Hexdump(packet.Data);
                    break;
            }
        }

        public void EnterWorld(string name, bool tutorial, bool goHome) {
            Send(AppPacket.Create(WorldOp.EnterWorld, new EnterWorld(name, tutorial, goHome)));
        }
    }
}
