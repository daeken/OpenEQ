namespace PacketRipper
{
    public enum OpCodes : ushort
    {
        OP_SessionRequest = 0x01,
        OP_SessionResponse = 0x02,
        OP_Combined = 0x03,
        OP_SessionDisconnect = 0x05,
        OP_KeepAlive = 0x06,
        OP_SessionStatRequest = 0x07,
        OP_SessionStatResponse = 0x08,
        OP_Packet = 0x09,
        OP_Fragment = 0x0d,
        OP_OutOfOrderAck = 0x11,
        OP_Ack = 0x15,
        OP_AppCombined = 0x19,
        OP_OutOfSession = 0x1d
    }
}