
namespace PacketRipper
{
    using System;

    public class EQRawApplicationPacket : EQApplicationPacket
    {
        //the actual raw EQ opcode
        public UFOpCodes opcode;

        public EQRawApplicationPacket(byte[] buf, int offset, int len) : base(buf, offset, len)
        {
            opcode = (UFOpCodes) BitConverter.ToUInt16(buf, offset); //*((const uint16 *) buf);
            if (opcode == 0x0000)
            {
                if (len >= 3)
                {
                    opcode = (UFOpCodes) BitConverter.ToUInt16(buf, 1);

                    // Skip the offset and the 3 byte opcode.
                    var newOffset = offset + 3;

                    // New length since this is a 3 byte opcode.
                    var packet_length = len - 3;

                    if (packet_length >= 0)
                    {
                        this.size = packet_length;
                        pBuffer = new byte[((BasePacket) this).size];
                        Buffer.BlockCopy(buf, newOffset, pBuffer, 0, pBuffer.Length);
                    }
                    else
                    {
                        size = 0;
                    }
                }
                else
                {
                    size = 0;
                }
            }
        }
    }
}