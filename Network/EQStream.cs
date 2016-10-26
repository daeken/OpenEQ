using System;
using static System.Console;
using static OpenEQ.Utility;
using System.Threading;

namespace OpenEQ.Network {
    public abstract class EQStream {
        public bool Compressing, Validating;
        public byte[] CRCKey;
        public ushort OutSequence, InSequence;

        AsyncUDPConnection conn;
        uint sessionID;

        ushort lastAcked = 65535;
        Packet[] sentPackets = new Packet[65536];
        Packet[] futurePackets = new Packet[65536];

        public EQStream(string host, int port) {
            conn = new AsyncUDPConnection(host, port);
            conn.Receive += Receive;

            new Thread(Checker).Start();
        }

        public void Connect() {
            var sr = new SessionRequest((uint) new Random().Next());
            sessionID = sr.SessionID;
            Send(Packet.Create(SessionOp.Request, sr, bare: true));
        }

        void Checker() {
            while(true) {
                var last = lastAcked + 1; // In case this changes in mid-stream; no need to lock
                for(var i = last; i < last + 65536; ++i) {
                    var packet = sentPackets[i % 65536];
                    if(packet == null || packet.Acked)
                        break;
                    if(Time.Now - packet.SentTime > 5) {
                        WriteLine($"Packet {packet.Sequence} not acked in {Time.Now - packet.SentTime}; resending.");
                        Send(packet);
                    }
                }
                Thread.Sleep(1000);
            }
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
                case SessionOp.Ack:
                    if(lastAcked > packet.Sequence)
                        for(var i = lastAcked + 1; i <= packet.Sequence + 65536; ++i)
                            sentPackets[i % 65536].Acked = true;
                    else
                        for(var i = lastAcked + 1; i <= packet.Sequence; ++i)
                            sentPackets[i].Acked = true;
                    lastAcked = packet.Sequence;
                    WriteLine($"Acked up to {packet.Sequence}");
                    break;
                default:
                    WriteLine($"Unknown packet received: {op} (0x{packet.Opcode:X04})");
                    break;
            }
        }

        protected void Send(Packet packet) {
            if(packet.SentTime == 0 && !packet.Bare && (packet.Opcode != (ushort) SessionOp.Ack || packet.Opcode != (ushort) SessionOp.Stats)) {
                packet.Sequence = OutSequence;
                sentPackets[OutSequence++] = packet;
            }
            packet.SentTime = Time.Now;
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
