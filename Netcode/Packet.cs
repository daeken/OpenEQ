using System;
using System.IO;
using System.Runtime.InteropServices;
using Ionic.Zlib;
using static OpenEQ.Netcode.Utility;

namespace OpenEQ.Netcode {
    public class Packet {
        public bool Bare;
        public ushort Opcode;
        public byte[] Data;
        public bool Acked = false;
        public float SentTime;
        public bool Valid = true;
        public byte[] Baked;
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
            Baked = packet;

            Opcode = packet.NetU16(0);
            var off = 2;
            switch((SessionOp) Opcode) {
                case SessionOp.Ack:
                case SessionOp.OutOfOrder:
                case SessionOp.Single:
                case SessionOp.Fragment:
                case SessionOp.Combined:
                    var plen = packet.Length - off;
                    if(!combined && stream.Validating) {
                        plen -= 2;
                        var mcrc = CalculateCRC(packet.Sub(0, packet.Length - 2), stream.CRCKey);
                        var pcrc = packet.NetU16(packet.Length - 2);
                        Valid = mcrc == pcrc;
                    }
                    if(!combined && stream.Compressing) {
                        if(packet[off] == 0x5a) {
							using(var ms = new MemoryStream(packet, 3, packet.Length - 3 - 2)) {
								using(var ds = new ZlibStream(ms, CompressionMode.Decompress)) {
									using(var tms = new MemoryStream()) {
										ds.CopyTo(tms);
										packet = tms.ToArray();
										plen = packet.Length;
										off = 0;
									}
								}
							}
                        } else if(packet[off] == 0xa5) {
                            off++;
                            plen--;
                        }
                    }
                    if((SessionOp) Opcode != SessionOp.Combined) {
                        Sequence = packet.NetU16(off);
                        off += 2;
                        plen -= 2;
                    }
                    Data = packet.Sub(off, off + plen);
                    Bare = false;
                    break;
				case SessionOp.Response:
					Data = packet.Sub(2);
					break;
			    case SessionOp.KeepAlive:
			        Opcode = (ushort) SessionOp.KeepAlive;
			        Bare = false;
			        break;
                default:
					Opcode = (ushort) SessionOp.Bare;
					if(packet.Length > 2 && packet[1] == 0xA5) {
						Data = packet.Sub(1);
						Data[0] = packet[0];
					} else
	                    Data = packet;
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
            val = Marshal.PtrToStructure<T>(i);
            Marshal.FreeHGlobal(i);
            return val;
        }

        public virtual byte[] Bake(EQStream stream) {
            if(Baked != null)
                return Baked;
            
            if(Bare) {
                Baked = new byte[Data.Length + 2];
                Baked[0] = (byte) (Opcode >> 8);
                Baked[1] = (byte) Opcode;
                Array.Copy(Data, 0, Baked, 2, Data.Length);
            } else {
                var len = Data.Length + 2 + 2;
                if(stream.Compressing)
                    len++;
                if(sequenced)
                    len += 2;
                Baked = new byte[len];
                Baked[0] = (byte) (Opcode >> 8);
                Baked[1] = (byte) Opcode;
                var off = 2;

                var doCompress = false;

                if(!doCompress && stream.Compressing)
                    Baked[off++] = 0xa5;

                if(sequenced) {
                    Baked[off++] = (byte)(sequence >> 8);
                    Baked[off++] = (byte)sequence;
                }
                Array.Copy(Data, 0, Baked, off, Data.Length);
                off += Data.Length;

                if(doCompress && stream.Compressing) {
                    using(var ms = new MemoryStream()) {
                        using(var ds = new ZlibStream(ms, CompressionMode.Compress)) {
                            ds.Write(Baked, 2, off - 2);
                            ds.Flush();
                        }
                        var temp = ms.ToArray();
                        var temp2 = new byte[3 + temp.Length + 2];
                        Array.Copy(Baked, temp2, 3);
                        temp2[2] = 0x5a;
                        Array.Copy(temp, 0, temp2, 3, temp.Length);
                        Baked = temp2;
                        off = temp.Length + 3;
                    }
                }
                if(stream.Validating) {
                    var crc = CalculateCRC(Baked.Sub(0, off), stream.CRCKey);
                    Baked[off++] = (byte) (crc >> 8);
                    Baked[off++] = (byte) crc;
                } else {
                    Baked[off++] = 0;
                    Baked[off++] = 0;
                }
            }
            return Baked;
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
            if(data[0] == 0) {
				if(data[1] == 0 && data.Length == 2) {
					Opcode = 0;
					Data = new byte[0];
				} else {
					Opcode = (ushort) (data[1] | (data[2] << 8));
					if(data.Length == 3)
						Data = new byte[0];
					else
						Data = data.Sub(3);
				}
            } else {
                Opcode = (ushort)(data[0] | (data[1] << 8));
				if(data.Length == 2)
					Data = new byte[0];
				else
					Data = data.Sub(2);
			}
		}

        public static AppPacket Create<OpT>(OpT opcode, byte[] data = null) {
            return new AppPacket((ushort) (object) opcode, data);
        }

        public static AppPacket Create<OpT, DataT>(OpT opcode, DataT data, byte[] extraData = null) where DataT : IEQStruct {
            var adata = data.Pack();
            if(extraData != null) {
                var arr = new byte[adata.Length + extraData.Length];
                Array.Copy(adata, 0, arr, 0, adata.Length);
                Array.Copy(extraData, 0, arr, adata.Length, extraData.Length);
                return new AppPacket((ushort) (object) opcode, arr);
            }

            return new AppPacket((ushort) (object) opcode, adata);
        }

        public T Get<T>() where T : IEQStruct, new() {
            var val = new T();
            val.Unpack(Data);
            return val;
        }
    }
}
