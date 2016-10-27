using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using static System.Console;
using static OpenEQ.Utility;

namespace OpenEQ.Network {
    public class Packet {
        public bool Bare;
        public ushort Opcode;
        public byte[] Data;
        public bool Acked = false;
        public float SentTime;
        public bool Valid = true;
        protected byte[] baked;
        bool sequenced;
        ushort sequence;

        public ushort Sequence {
            get { return sequence; }
            set {
                sequenced = true;
                sequence = value;
            }
        }

        public Packet(ushort opcode, byte[] data, bool bare = false) {
            Bare = bare;
            Opcode = opcode;
            Data = data;
        }

        public Packet(SessionOp opcode, byte[] data, bool bare = false) : this((ushort) opcode, data, bare) { }

        public Packet(EQStream stream, byte[] packet, bool combined = false) {
            baked = packet;

            Opcode = packet.NetU16(0);
            var off = 2;
            switch((SessionOp) Opcode) {
                case SessionOp.Ack:
                case SessionOp.OutOfOrder:
                case SessionOp.Single:
                case SessionOp.Fragment:
                case SessionOp.Combined:
                    if((SessionOp) Opcode != SessionOp.Combined) {
                        Sequence = packet.NetU16(off);
                        off += 2;
                    }
                    var plen = packet.Length - off;
                    if(!combined && stream.Validating) {
                        plen -= 2;
                        var mcrc = CalculateCRC(packet.Sub(0, packet.Length - 2), stream.CRCKey);
                        var pcrc = packet.NetU16(packet.Length - 2);
                        Valid = mcrc == pcrc;
                    }
                    if(!combined && stream.Compressing) {
                        if(packet[off] == 0x5a) {
                            WriteLine("Compressed packet :(");
                        } else {
                            Debug.Assert(packet[off] == 0xa5);
                            off++;
                            plen--;
                            Data = packet.Sub(off, off + plen);
                        }
                    } else
                        Data = packet.Sub(off, off + plen);
                    Bare = false;
                    break;
                default:
                    Data = packet.Sub(2);
                    Bare = true;
                    break;
            }
        }

        public static Packet Create<OpT>(OpT opcode, ushort sequence = 0) {
            return new Packet((ushort) (object) opcode, new byte[0]) { Sequence = sequence };
        }

        public static Packet Create<OpT, DataT>(OpT opcode, DataT data, bool bare = false) {
            var size = Marshal.SizeOf(data);
            var arr = new byte[size];

            var ptr = Marshal.AllocHGlobal(size);
            Marshal.StructureToPtr(data, ptr, true);
            Marshal.Copy(ptr, arr, 0, size);
            Marshal.FreeHGlobal(ptr);
            return new Packet((ushort) (object) opcode, arr, bare);
        }
        
        public T Get<T>() where T : struct {
            var val = new T();
            int len = Marshal.SizeOf(val);
            var i = Marshal.AllocHGlobal(len);
            Marshal.Copy(Data, 0, i, len);
            val = (T) Marshal.PtrToStructure(i, val.GetType());
            Marshal.FreeHGlobal(i);
            return val;
        }

        public virtual byte[] Bake(EQStream stream) {
            if(baked != null)
                return baked;
            
            if(Bare) {
                baked = new byte[Data.Length + 2];
                baked[0] = (byte) (Opcode >> 8);
                baked[1] = (byte) Opcode;
                Array.Copy(Data, 0, baked, 2, Data.Length);
            } else {
                var len = Data.Length + 2 + 2;
                if(stream.Compressing)
                    len++;
                if(sequenced)
                    len += 2;
                baked = new byte[len];
                baked[0] = (byte) (Opcode >> 8);
                baked[1] = (byte) Opcode;
                var off = 2;
                if(stream.Compressing)
                    baked[off++] = 0xa5;
                if(sequenced) {
                    baked[off++] = (byte) (sequence >> 8);
                    baked[off++] = (byte) sequence;
                }
                Array.Copy(Data, 0, baked, off, Data.Length);
                off += Data.Length;
                if(stream.Validating) {
                    var crc = CalculateCRC(baked.Sub(0, off), stream.CRCKey);
                    baked[off++] = (byte) (crc >> 8);
                    baked[off++] = (byte) crc;
                } else {
                    baked[off++] = 0;
                    baked[off++] = 0;
                }
            }
            return baked;
        }
    }

    public class AppPacket {
        public ushort Opcode;
        public byte[] Data;

        public int Size => (Data != null ? Data.Length : 0) + 2;

        public AppPacket(ushort opcode, byte[] data = null) {
            Opcode = opcode;
            Data = data;
        }

        public AppPacket(byte[] data) {
            Opcode = (ushort) (data[0] | (data[1] << 8));
            Data = data.Sub(2);
        }

        public static AppPacket Create<OpT>(OpT opcode, byte[] data = null) {
            return new AppPacket((ushort) (object) opcode, data);
        }

        public static AppPacket Create<OpT, DataT>(OpT opcode, DataT data, byte[] extraData = null) {
            var size = Marshal.SizeOf(data);
            var arr = new byte[size + (extraData != null ? extraData.Length : 0)];

            var ptr = Marshal.AllocHGlobal(size);
            Marshal.StructureToPtr(data, ptr, true);
            Marshal.Copy(ptr, arr, 0, size);
            Marshal.FreeHGlobal(ptr);

            if(extraData != null)
                Array.Copy(extraData, 0, arr, size, extraData.Length);

            return new AppPacket((ushort) (object) opcode, arr);
        }

        public T Get<T>() where T : struct {
            var val = new T();
            int len = Marshal.SizeOf(val);
            var i = Marshal.AllocHGlobal(len);
            Marshal.Copy(Data, 0, i, len);
            val = (T) Marshal.PtrToStructure(i, val.GetType());
            Marshal.FreeHGlobal(i);
            return val;
        }
    }
}
