
namespace PacketRipper
{
    using System;
    using PacketRipper.Extensions;

    public class EQProtocolPacket : BasePacket
    {
        private ushort _opCode;

        /// <summary>
        /// TODO: Make opcode of type OpCodes to remove unnecessary casting.
        /// </summary>
        public ushort opcode
        {
            get { return _opCode; }
            set
            {
                _opCode = value;

                // Set this to eliminate some unnecessary casting later on.
                OpCodeEnum = (OpCodes)value;
            }
        }

        public OpCodes OpCodeEnum { get; private set; }

        public bool acked;
        public uint sent_time;

        public EQProtocolPacket(ushort op, byte[] buf, int len) : base(buf, len)
        {
            opcode = op;
            acked = false;
            sent_time = 0;
        }

        public EQProtocolPacket(ushort op, byte[] buf, int offset, int len) : base(buf, offset, len)
        {
            opcode = op;
            acked = false;
            sent_time = 0;
        }

        public ushort GetRawOpcode()
        {
            return opcode;
        }

        public int Size()
        {
            return size + 2;
        }

        public bool combine(EQProtocolPacket rhs)
        {
            bool result = false;
            if (opcode == (ushort)OpCodes.OP_Combined && size + rhs.size + 5 < 256)
            {
                // This packet was previously combined.  Add another packet into it.
                byte[] tmpbuffer = new byte[size + rhs.size + 3];

                // Copy the current packet (left-hand side) into the new buffer.
                Buffer.BlockCopy(pBuffer, 0, tmpbuffer, 0, (int)size);

                // set the offset for the second packet to the end of the first.
                var offset = size;

                // add the size of the second packet.
                Buffer.BlockCopy(BitConverter.GetBytes(rhs.Size()), 0, tmpbuffer, (int)offset++, sizeof(ushort));

                // Copy in the second packet.
                offset += rhs.serialize(tmpbuffer, offset);

                // update the total size.
                size = offset;
                // Copy the combined packet into this one.
                pBuffer = tmpbuffer;
                result = true;
            }
            else if (size + rhs.size + 7 < 256)
            {
                // This packet hasn't been combined before, so once we're done mark it as a combined packet.
                var tmpbuffer = new byte[size + rhs.size + 6];
                var offset = 0;

                // Explanation of the shenenagians here.  Size() is uint which is 4 bytes in size, so the byte
                // array produced by GetBytes is length 4.  However, we only increase the offset by 1 (1 byte)
                // because we don't NEED all 4 bytes, we only need the first byte (due to the small size of packets
                // and Intel CPUs being little endian.
                Buffer.BlockCopy(BitConverter.GetBytes(Size()), 0, tmpbuffer, offset++, sizeof(uint)); // shouldn't this be an offset of sizeof(uint)?
                offset += serialize(tmpbuffer, offset);
                Buffer.BlockCopy(BitConverter.GetBytes(rhs.Size()), 0, tmpbuffer, offset++, sizeof(uint));
                offset += rhs.serialize(tmpbuffer, offset);

                size = offset;
                pBuffer = tmpbuffer;
                opcode = (ushort)OpCodes.OP_Combined;
                result = true;
            }

            return result;
        }

        public int serialize(byte[] dest, int offset)
        {
            if (opcode > 0xFF)
            {
                Buffer.BlockCopy(BitConverter.GetBytes(opcode), 0, dest, offset, sizeof(ushort));
                offset += 2;    // the size of the opcode.
            }
            else
            {
                dest[offset++] = 0x00;
                Buffer.BlockCopy(BitConverter.GetBytes(opcode), 0, dest, offset++, sizeof(ushort));
            }

            // Now copy the packet into the buffer.
            Buffer.BlockCopy(pBuffer, 0, dest, offset++, size);

            return size + 2;
        }

        public EQProtocolPacket Copy()
        {
            return new EQProtocolPacket(opcode, pBuffer, size);
        }

        public static bool ValidateCRC(byte[] buffer, int length, uint key)
        {
            bool valid = false;
            // OP_SessionRequest, OP_SessionResponse, OP_OutOfSession are not CRC'd
            // TODO: Now we're using OpCodes as Byte values instead of ushort?  Can't they just be bytes??
            if (buffer[0] == 0x00 &&
                (buffer[1] == (byte)OpCodes.OP_SessionRequest ||
                 buffer[1] == (byte)OpCodes.OP_SessionResponse ||
                 buffer[1] == (byte)OpCodes.OP_OutOfSession))
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

        public static int Compress(byte[] buffer, int length, ref byte[] newbuf, int newbufsize)
        {
            int flag_offset = 1;
            int newlength;

            newbuf[0] = buffer[0];
            if (buffer[0] == 0)
            {
                flag_offset = 2;
                newbuf[1] = buffer[1];
            }
            if (length > 30)
            {
                newlength = SharpZip.DeflatePacket(buffer, flag_offset, length - flag_offset, newbuf, flag_offset + 1);
                newbuf[flag_offset] = 0x5a;
                newlength += flag_offset + 1;
                var tmpbuf = new byte[newlength];
                Buffer.BlockCopy(newbuf, 0, tmpbuf, 0, newlength);
                newbuf = tmpbuf;
            }
            else
            {
                // Not large enough to bother compressing.  Inject the Not Compessed indicator into the array.
                newlength = length + 1;
                var tmpbuf = new byte[newlength];
                // Copy the data before the point where we will insert the indicator.
                Buffer.BlockCopy(newbuf, 0, tmpbuf, 0, flag_offset);
                // Add the indicator.
                tmpbuf[flag_offset] = 0xa5;
                // Now copy the rest.
                Buffer.BlockCopy(buffer, flag_offset, tmpbuf, flag_offset + 1, length - flag_offset);
                newbuf = tmpbuf;
            }

            return newlength;
        }

        public static void ChatDecode(ref byte[] buffer, int size, int DecodeKey)
        {
            if ((size >= 2) && buffer[1] != 0x01 && buffer[0] != 0x02 && buffer[0] != 0x1d)
            {
                int Key = DecodeKey;
                var offset = 2;
                size -= 2;
                byte[] test = new byte[size];

                int i;
                for (i = 0; i + 4 <= size; i += 4)
                {
                    int pt = BitConverter.ToInt32(buffer, offset + i) ^ (Key);
                    Key = BitConverter.ToInt32(buffer, offset + i);
                    Buffer.BlockCopy(BitConverter.GetBytes(pt), 0, test, i, sizeof(int));
                }
                byte KC = (byte)(Key & 0xFF);
                for (; i < size; i++)
                {
                    test[i] = BitConverter.GetBytes((buffer[offset + i] ^ KC))[0];
                }

                buffer = test;
            }
        }

        public static void ChatEncode(ref byte[] buffer, int size, uint EncodeKey)
        {
            if (buffer[1] != 0x01 && buffer[0] != 0x02 && buffer[0] != 0x1d)
            {
                int Key = (int)EncodeKey;
                var offset = 2;
                size -= 2;
                // This was of type sbyte.  Given how many fucking casts back and forth are in this emulator it probably
                // was because some jackass was like, "sbytes dude, that's how it should be done" when it didn't fucking matter.
                byte[] test = new byte[size];

                int i;
                for (i = 0; i + 4 <= size; i += 4)
                {
                    int pt = BitConverter.ToInt32(buffer, offset + i) ^ (Key);
                    Key = pt;
                    Buffer.BlockCopy(BitConverter.GetBytes(pt), 0, test, i, sizeof(int));
                }
                byte KC = (byte)(Key & 0xFF);
                for (; i < size; i++)
                {
                    test[i] = BitConverter.GetBytes((buffer[offset + i] ^ KC))[0];
                }

                buffer = test;
            }
        }
    }
}
