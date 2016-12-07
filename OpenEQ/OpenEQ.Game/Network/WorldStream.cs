using System;
using System.Collections.Generic;
using System.Text;
using OpenEQ.Chat;
using static System.Console;
using static OpenEQ.Network.Utility;

namespace OpenEQ.Network {
    public class WorldStream : EQStream {
        public event EventHandler<List<CharacterSelectEntry>> CharacterList;
        public event EventHandler<byte> CharacterCreateNameApproval;
        public event EventHandler<string> MOTD;
        public event EventHandler<ZoneServerInfo> ZoneServer;
        public event EventHandler<byte[]> ChatServerList;

        public List<ChatServer> ChatServers;

        uint accountID;
        string sessionKey;

        public WorldStream(string host, int port, uint accountID, string sessionKey) : base(host, port) {
            ChatServers = new List<ChatServer>();

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
            switch((WorldOp) packet.Opcode) {
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
                case WorldOp.SetChatServer:
                case WorldOp.SetChatServer2:
                    ChatServerList?.Invoke(this, packet.Data);
                    break;
                case WorldOp.PostEnterWorld:
                    // The emu doesn't do anything with ApproveWorld and WorldClientReady so we may be able to just skip them both.
                    Send(AppPacket.Create(WorldOp.ApproveWorld, null));
                    Send(AppPacket.Create(WorldOp.WorldClientReady, null));
                    break;
                case WorldOp.ApproveName:
                    CharacterCreateNameApproval?.Invoke(this, packet.Data[0]);
                    break;
                default:
                    WriteLine($"Unhandled packet in WorldStream: {(WorldOp)packet.Opcode} (0x{packet.Opcode:X04})");
                    Hexdump(packet.Data);
                    break;
            }
        }

        public void SendNameApproval(NameApproval nameApproval)
        {
            Send(AppPacket.Create(WorldOp.ApproveName, nameApproval));
        }

        public void SendCharacterCreate(CharCreate charCreate)
        {
            Send(AppPacket.Create(WorldOp.CharacterCreate, charCreate));
        }

        public void SendEnterWorld(EnterWorld enterWorld)
        {
            Send(AppPacket.Create(WorldOp.EnterWorld, enterWorld));
        }
    }
}
