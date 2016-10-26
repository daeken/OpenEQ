using System;
using static System.Console;
using static OpenEQ.Utility;

namespace OpenEQ.Network {
    public abstract class EQStream {
        public bool Compressing, Validating;

        AsyncUDPConnection conn;
        uint sessionID;
        
        public EQStream(string host, int port) {
            conn = new AsyncUDPConnection(host, port);
            conn.Receive += Receive;
        }

        public void Connect() {
            var sr = new SessionRequest((uint) new Random().Next());
            sessionID = sr.SessionID;
            Send(Packet.CreatePacket(SessionOp.Request, sr, bare: true));
        }

        void Receive(object sender, byte[] data) {
            var packet = new Packet(this, data);
            var op = (SessionOp) packet.Opcode;
            switch(op) {
                case SessionOp.Response:
                    var response = packet.Get<SessionResponse>();
                    Compressing = (response.filterMode & FilterMode.Compressed) != 0;
                    Validating = (response.validationMode & ValidationMode.Crc) != 0;
                    HandleSessionResponse(packet);
                    break;
                default:
                    WriteLine($"Unknown packet received: {op} (0x{packet.Opcode:X04})");
                    break;
            }
        }

        void Send(Packet packet) {
            conn.Send(packet.RawData);
        }

        void SendRaw(byte[] packet) {
            conn.Send(packet);
        }

        protected abstract void HandleSessionResponse(Packet packet);
    }
}
