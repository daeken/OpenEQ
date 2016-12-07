
namespace PacketRipper
{
    using System;
    using OpenEQ.Network;

    public class EQStream
    {
        private const int Key = 0x11223344;
        private bool compressed = true;

        public EQProtocolPacket Process(byte[] buffer, int length)
        {
            var newbuffer = new byte[2048];
            var newlength = 0;

            if (Packet.ValidateCRC(buffer, length, Key))
            {
                if (compressed)
                {
                    newlength = Packet.Decompress(buffer, length, ref newbuffer, newbuffer.Length);
                }
                else
                {
                    Buffer.BlockCopy(buffer, 0, newbuffer, 0, length);
                    newlength = length;

                    //if (encoded)
                    //    EQProtocolPacket.ChatDecode(ref newbuffer, newlength - 2, (int)Key);
                }

                if (buffer[1] != 0x01 && buffer[1] != 0x02 && buffer[1] != 0x1d)
                    newlength -= 2;

                return MakeProtocolPacket(newbuffer, newlength);

                //ProcessPacket(p);
                //ProcessQueue();
            }
            else
            {
                Console.WriteLine("Incoming packet failed checksum");
            }

            return null;
        }

        public static EQProtocolPacket MakeProtocolPacket(byte[] buf, int len)
        {
            return MakeProtocolPacket(buf, 0, len);
        }

        public static EQProtocolPacket MakeProtocolPacket(byte[] buf, int offset, int len)
        {
            // I think this is "take the ushort bytes, aka first two
            var proto_opcode = buf.NetU16(offset);//BitConverter.ToUInt16(buf, offset).NToHus()); //ntohs(*(const ushort* )buf);

            // advance over opcode and shrink the remaining packet length by 2.
            return new EQProtocolPacket(proto_opcode, buf, offset + 2, len - 2);
        }

        public static EQRawApplicationPacket MakeApplicationPacket(byte[] buf, int len)
        {
            return MakeApplicationPacket(buf, 0, len);
        }

        public static EQRawApplicationPacket MakeApplicationPacket(byte[] buf, int offset, int len)
        {
            //Log.DetailRule.Debug($"Creating new application packet, length {len} starting at offset {offset}");
            //System.Diagnostics.Trace.WriteLine($"buf: {BitConverter.ToString(buf)}, offset: {offset}, len: {len}");
            var ap = new EQRawApplicationPacket(buf, offset, len);
            //System.Diagnostics.Trace.WriteLine($"buf: {BitConverter.ToString(ap.pBuffer)}, ap.GetRawOpcode: {ap.GetRawOpcode()}");
            return ap;
        }
    }
}
