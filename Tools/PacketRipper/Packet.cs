using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection.Emit;
using System.Text;
using System.Threading.Tasks;
using OpenEQ.Network;
using PacketRipper.Extensions;

namespace PacketRipper
{
    public class Packet
    {
        public SessionOp OpCode;
        public byte[] Data;
        public ushort Sequence;
        public bool hasValidCrc;

        public Packet(byte[] input, int offset, int dataLength)
        {
            // Get the opcode.
            OpCode = (SessionOp)input.NetU16(offset);
            var localOffset = sizeof(ushort);

            // If this is a single packet, get the sequence # too.
            if (SessionOp.Single == OpCode)
            {
                Sequence = input.NetU16(offset);
                localOffset += sizeof(ushort);
            }

            if (dataLength <= localOffset) return;

            Data = new byte[dataLength - localOffset];
            Buffer.BlockCopy(input, offset + localOffset, Data, 0, Data.Length);
        }

        public static bool ValidateCRC(byte[] buffer, int length, uint key)
        {
            var valid = false;

            // OP_SessionRequest, OP_SessionResponse, OP_OutOfSession are not CRC'd
            // TODO: Now we're using OpCodes as Byte values instead of ushort?  Can't they just be bytes??
            if (buffer[0] == 0x00 &&
                (buffer[1] == (byte)SessionOp.Request||
                 buffer[1] == (byte)SessionOp.Response))// ||
                 //buffer[1] == (byte)SessionOp.Codes.OP_OutOfSession))
            {
                valid = true;
            }
            else
            {
                return buffer.ValidateCRC(key);
            }

            return valid;
        }

        public static int Decompress(byte[] buffer, int length, ref byte[] newbuf, int newbufsize)
        {
            var newlen = 0;
            var flag_offset = 0;
            newbuf[0] = buffer[0];
            if (buffer[0] == 0x00)
            {
                flag_offset = 2;
                newbuf[1] = buffer[1];
            }
            else
            {
                flag_offset = 1;
            }

            if (length > 2 && buffer[flag_offset] == 0x5a)
            {
                // TODO: Decompress is broken I suspect.
                // This packet is compressed, so decompress (Inflate) it (and add 2 at the end of this long fucking method call)
                newlen =
                    SharpZip.InflatePacket(buffer, flag_offset + 1, length - (flag_offset + 1) - 2, newbuf, flag_offset) + 2;
                newbuf[newlen++] = buffer[length - 2];
                newbuf[newlen++] = buffer[length - 1];
                // TODO: Fucked up for now until I get this under test and refactor.
                // Take our 2048 byte array and make it fit the data.  We're not doing any pointer shit here.
                var tmpBuff = new byte[newlen];
                Buffer.BlockCopy(newbuf, 0, tmpBuff, 0, newlen);
                // Now overwrite.
                newbuf = tmpBuff;
            }
            else if (length > 2 && buffer[flag_offset] == 0xa5)
            {
                // This packet is not compressed.  Just remove the compression indicator.
                Buffer.BlockCopy(buffer, flag_offset + 1, newbuf, flag_offset, length - (flag_offset + 1));
                newlen = length - 1;
                var tmpBuff = new byte[newlen];
                Buffer.BlockCopy(newbuf, 0, tmpBuff, 0, newlen);
                newbuf = tmpBuff;
            }
            else
            {
                // Nothing to do.
                // TODO: Copy it?  Could just reassign it probably?
                newbuf = buffer;
                newlen = length;
            }

            return newlen;
        }

    }
}