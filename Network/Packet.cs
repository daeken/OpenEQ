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

        public Packet(EQStream stream, byte[] packet) {
            baked = packet;

            Opcode = packet.NetU16(0);
            var off = 2;
            switch(Opcode) {
                case 0x15:
                case 0x11:
                case 0x09:
                case 0x0d:
                case 0x03:
                case 0x19:
                    Sequence = packet.NetU16(off);
                    off += 2;
                    var plen = packet.Length - off;
                    if(stream.Validating) {
                        plen -= 2;
                        // XXX: Check CRC
                    }
                    if(stream.Compressing) {
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

        public static Packet Create<T>(ushort opcode, T data, bool bare = false) {
            int size = Marshal.SizeOf(data);
            byte[] arr = new byte[size];

            IntPtr ptr = Marshal.AllocHGlobal(size);
            Marshal.StructureToPtr(data, ptr, true);
            Marshal.Copy(ptr, arr, 0, size);
            Marshal.FreeHGlobal(ptr);
            return new Packet((ushort) opcode, arr, bare);
        }

        public static Packet Create<T>(SessionOp opcode, T data, bool bare = false) {
            return Create((ushort) opcode, data, bare);
        }

        public T Get<T>() where T : struct {
            var val = new T();
            int len = Marshal.SizeOf(val);
            IntPtr i = Marshal.AllocHGlobal(len);
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
                }
            }
            return baked;
        }
    }

    public class AppPacket {
        public ushort Opcode;
        public byte[] Data;

        public int Size => Data.Length + 2;

        public AppPacket(ushort opcode, byte[] data) {
            Opcode = opcode;
            Data = data;
        }
        public AppPacket(LoginOp opcode, byte[] data) : this((ushort) opcode, data) { }
        public static AppPacket Create<T>(ushort opcode, T data) {
            int size = Marshal.SizeOf(data);
            byte[] arr = new byte[size];

            IntPtr ptr = Marshal.AllocHGlobal(size);
            Marshal.StructureToPtr(data, ptr, true);
            Marshal.Copy(ptr, arr, 0, size);
            Marshal.FreeHGlobal(ptr);
            return new AppPacket((ushort) opcode, arr);
        }
        public static AppPacket Create<T>(LoginOp opcode, T data) {
            return Create((ushort) opcode, data);
        }
    }
}
