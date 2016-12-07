
namespace PacketRipper
{
    using System;

    public class BasePacket
    {
        public byte[] pBuffer;
        public int size;

        /// <summary>
        /// TODO: We don't need to pass length.  Just get it from the buffer.
        /// </summary>
        /// <param name="buff"></param>
        /// <param name="len"></param>
        public BasePacket(byte[] buff, int len)
        {
            pBuffer = buff;
            size = len;
        }

        public BasePacket(byte[] buff, int offset, int len)
        {
            pBuffer = new byte[len];
            Buffer.BlockCopy(buff, offset, pBuffer, 0, len);
            size = pBuffer.Length;
        }
    }
}