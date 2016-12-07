
namespace PacketRipper
{
    using System;
    using OpenEQ.Network;

    public class LoginPacket
    {
        public LoginOp OpCode;
        public byte[] Data;

        public LoginPacket(byte[] input)
        {
            // Get the opcode.
            OpCode = (LoginOp) BitConverter.ToUInt16(input, 0);

            Data = new byte[input.Length - sizeof(ushort)];
            Buffer.BlockCopy(input, sizeof(ushort), Data, 0, Data.Length);
        }
    }
}