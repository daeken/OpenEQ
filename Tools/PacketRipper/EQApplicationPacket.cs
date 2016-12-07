
namespace PacketRipper
{
    public class EQApplicationPacket : BasePacket // : EQPacket
    {
        public EQApplicationPacket(byte[] buff, int len) : base(buff, len)
        {
        }

        public EQApplicationPacket(byte[] buff, int offset, int len) : base(buff, offset, len)
        {
        }
    }
}