using System;
using static System.Console;
using static OpenEQ.Utility;

namespace OpenEQ.Network {
    public abstract class EQStream {
        public bool Compressing, Validating;
        public byte[] CRCKey;
        public ushort OutSequence, InSequence;

        AsyncUDPConnection conn;
        uint sessionID;
        
        public EQStream(string host, int port) {
            conn = new AsyncUDPConnection(host, port);
            conn.Receive += Receive;
        }

        public void Connect() {
            var sr = new SessionRequest((uint) new Random().Next());
            sessionID = sr.SessionID;
            Send(Packet.Create(SessionOp.Request, sr, bare: true));
        }

        void Receive(object sender, byte[] data) {
            var packet = new Packet(this, data);
            var op = (SessionOp) packet.Opcode;
            switch(op) {
                case SessionOp.Response:
                    var response = packet.Get<SessionResponse>();
                    Compressing = (response.filterMode & FilterMode.Compressed) != 0;
                    Validating = (response.validationMode & ValidationMode.Crc) != 0;
                    CRCKey = new byte[] {
                        (byte) (response.crcKey >> 24),
                        (byte) (response.crcKey >> 16),
                        (byte) (response.crcKey >> 8),
                        (byte) response.crcKey
                    };
                    HandleSessionResponse(packet);
                    break;
                default:
                    WriteLine($"Unknown packet received: {op} (0x{packet.Opcode:X04})");
                    break;
            }
        }

        protected void Send(Packet packet) {
            if(!packet.Bare && (packet.Opcode != (ushort) SessionOp.Ack || packet.Opcode != (ushort) SessionOp.Stats)) {
                packet.Sequence = OutSequence;
            }
            conn.Send(packet.Bake(this));
        }

        protected void Send(AppPacket packet) {
            if(packet.Size > 512 - 7) { // Fragment

            } else {
                var data = new byte[packet.Size];
                data[0] = (byte) (packet.Opcode >> 8);
                data[1] = (byte) packet.Opcode;
                Array.Copy(packet.Data, 0, data, 2, packet.Data.Length);
                Send(new Packet(SessionOp.Single, data));
            }
        }

        void SendRaw(byte[] packet) {
            conn.Send(packet);
        }

        protected abstract void HandleSessionResponse(Packet packet);
    }
}
