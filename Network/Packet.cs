using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using static System.Console;

namespace OpenEQ.Network {
    public class Packet {
        bool bare;
        public ushort Opcode;
        public byte[] Data;
        byte[] baked;

        public byte[] RawData {
            get {
                if(baked != null)
                    return baked;
                return Bake();
            }
        }

        public Packet(ushort opcode, byte[] data, bool bare = false) {
            this.bare = bare;
            Opcode = opcode;
            Data = data;
        }

        public Packet(SessionOp opcode, byte[] data, bool bare = false) : this((ushort) opcode, data, bare) {}

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
                    }
                    bare = false;
                    break;
                default:
                    Data = packet.Sub(2);
                    WriteLine($"Foo {packet.Length} {Data.Length}");
                    bare = true;
                    break;
            }
        }

        public static Packet CreatePacket<T>(ushort opcode, T data, bool bare = false) {
            int size = Marshal.SizeOf(data);
            byte[] arr = new byte[size];

            IntPtr ptr = Marshal.AllocHGlobal(size);
            Marshal.StructureToPtr(data, ptr, true);
            Marshal.Copy(ptr, arr, 0, size);
            Marshal.FreeHGlobal(ptr);
            return new Packet((ushort) opcode, arr, bare);
        }

        public static Packet CreatePacket<T>(SessionOp opcode, T data, bool bare = false) {
            return CreatePacket((ushort) opcode, data, bare);
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

        byte[] Bake() {
            if(bare) {
                baked = new byte[Data.Length + 2];
                baked[0] = (byte) (Opcode >> 8);
                baked[1] = (byte) (Opcode & 0xFF);
                Array.Copy(Data, 0, baked, 2, Data.Length);
            }
            return baked;
        }
    }
}
