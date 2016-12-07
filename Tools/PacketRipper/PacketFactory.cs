using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using OpenEQ.Network;
using PacketRipper;
using PacketRipper.Extensions;

namespace PacketRipper
{
    public class PacketFactory
    {
        private static PacketFactory _packetFactory;
        private byte[] oversize_buffer;
        private int oversize_offset;
        private uint oversize_length;
        private ushort NextInSeq;
        private Dictionary<ushort, EQProtocolPacket> PacketQueue;

        public static PacketFactory Instance => _packetFactory ?? (_packetFactory = new PacketFactory());

        protected PacketFactory()
        {
            NextInSeq = 0;
            PacketQueue = new Dictionary<ushort, EQProtocolPacket>();
        }

        public void Process(string[] fields)
        {
            var tmp = fields[fields.Length - 1].StringToByteArray();

            ProcessPacket(fields[0], fields[1], new EQStream().Process(tmp, tmp.Length));
        }

        private void ProcessPacket(string source, string destination, EQProtocolPacket p)
        {
            int processed = 0;
            int subpacket_length = 0;

            if (null == p)
                return;

            //// Raw Application packet
            //if (p.opcode > 0xff)
            //{
            //    p.opcode = p.opcode.HToNus(); //byte order is backwards in the protocol packet
            //    var ap = MakeApplicationPacket(p);

            //    if (null != ap)
            //    {
            //        InboundQueue.TryAdd(ap);
            //    }

            //    return;
            //}

            //if (0 == Session && p.OpCodeEnum != OpCodes.OP_SessionRequest && p.OpCodeEnum != OpCodes.OP_SessionResponse)
            //{
            //    Log.DetailRule.Debug("Session not initialized, packet ignored");
            //    // _raw(NET__DEBUG, 0xFFFF, p);
            //    return;
            //}

            switch (p.OpCodeEnum)
            {
                case OpCodes.OP_Combined:
                {
                    processed = 0;
                    while (processed < p.size)
                    {
                        subpacket_length = p.pBuffer[processed];
                        // +1 advances past the packet size byte.
                        var subp = EQStream.MakeProtocolPacket(p.pBuffer, processed + 1, subpacket_length);

                        ProcessPacket(source, destination, subp);
                        //delete subp;
                        processed += subpacket_length + 1;
                    }
                }
                    break;

                case OpCodes.OP_AppCombined:
                {
                    processed = 0;
                    while (processed < p.size)
                    {
                        var ap = default(EQRawApplicationPacket);
                        if ((subpacket_length = p.pBuffer[processed]) != 0xff)
                        {
// (unsigned char)*(p->pBuffer + processed))!= 0xff) {
                            //Log.DetailRule.Debug($"Extracting combined app packet of length {subpacket_length}, short len");
                            // + 1 to skip past the packet size.
                            //System.Diagnostics.Trace.WriteLine($"p.pBuffer = {BitConverter.ToString(p.pBuffer)}, processed = {processed} + 1, subpacket_length: {subpacket_length}");
                            ap = EQStream.MakeApplicationPacket(p.pBuffer, processed + 1, subpacket_length);
                            //System.Diagnostics.Trace.WriteLine($"ap.GetRawOpcode = {ap.GetRawOpcode()}, ap.pBuffer = {BitConverter.ToString(ap.pBuffer)}");
                            processed += subpacket_length + 1;
                            //System.Diagnostics.Trace.WriteLine($"processed = {processed}");
                        }
                        else
                        {
                            // If the first byte of the size is 0xff then this is actually a two byte size, so skip the 0xff
                            // to then get the size.
                            subpacket_length = p.pBuffer.NetU16(processed + 1);
                                // BitConverter.ToUInt16(p.pBuffer, processed + 1).NToHus();//ntohs(*(ushort*)(p->pBuffer + processed + 1));
                            //Log.DetailRule.Debug($"Extracting combined app packet of length {subpacket_length}, short len");
                            // Skipping 3 because 1 is 0xff, the next two bytes are the size.
                            ap = EQStream.MakeApplicationPacket(p.pBuffer, processed + 3, subpacket_length);
                            processed += subpacket_length + 3;
                        }
                        if (null != ap)
                        {
                            Console.WriteLine(
                                $"{source} => {destination}, ApplicationPacket: OpCode {ap.opcode}, Data: {BitConverter.ToString(ap.pBuffer)}");
                            //ap.copyInfo(p);
                            //InboundQueue.TryAdd(ap);
                        }
                    }
                }
                    break;

                case OpCodes.OP_Packet:
                {
                    var seq = p.pBuffer.NetU16(0);

                    // Check for an embedded OP_AppCombinded (protocol level 0x19)
                    if (p.pBuffer[2] == 0x00 && p.pBuffer[3] == 0x19)
                        //if (*(p->pBuffer + 2) == 0x00 && *(p->pBuffer + 3) == 0x19)
                    {
                        var subp = EQStream.MakeProtocolPacket(p.pBuffer, 2, p.size - 2);

                        ProcessPacket(source, destination, subp);
                    }
                    else
                    {
                        var ap = EQStream.MakeApplicationPacket(p.pBuffer, 2, p.size - 2);
                        Console.WriteLine(
                            $"{source} => {destination}, ApplicationPacket: OpCode {ap.opcode}, Data: {BitConverter.ToString(ap.pBuffer)}");
                    }
                }
                    break;

                case OpCodes.OP_Fragment:
                {
                    if (null == p.pBuffer || (p.Size() < 4))
                    {
                        Console.WriteLine("Received OP_Fragment that was of malformed size");
                        break;
                    }
                    var seq = p.pBuffer.NetU16(0);

                    // In case we did queue one before as well.
                    if (PacketQueue.Remove(seq))
                    {
                        Console.WriteLine($"[NET_TRACE] OP_Fragment: Removing older queued packet with sequence {seq}");
                        //delete qp;
                    }
                    NextInSeq++;
                    if (null != oversize_buffer)
                    {
                        Buffer.BlockCopy(p.pBuffer, 2, oversize_buffer, oversize_offset, p.size - 2);
                        oversize_offset += p.size - 2;

                        // Does the offset match the total length?  If so it means we have assembled the full packet.  If not,
                        // that means we're still waiting for all of the parts to arrive.
                        if (oversize_offset == oversize_length)
                        {
                            // We have the full packet.  What kind is it?
                            if (p.pBuffer[2] == 0x00 && p.pBuffer[3] == 0x19)
                            {
                                var subp = EQStream.MakeProtocolPacket(oversize_buffer, oversize_offset);
                                ProcessPacket(source, destination, subp);
                            }
                            else
                            {
                                var ap = EQStream.MakeApplicationPacket(oversize_buffer, oversize_offset);
                                if (null != ap)
                                {
                                    Console.WriteLine($"{source} => {destination}, {ap.opcode}");
                                }
                            }
                        }
                    }
                    else
                    {
                        // Allocate space for the fragment.  The first fragment contains the total size of the
                        // fragment.
                        oversize_length = p.pBuffer.NetU32(2);
                            // BitConverter.ToUInt32(p.pBuffer, 2).NToHui();//ntohl(*(uint*)(p->pBuffer + 2));
                        oversize_buffer = new byte[oversize_length];
                        Buffer.BlockCopy(p.pBuffer, 6, oversize_buffer, 0, p.size - 6);
                            //memcpy(oversize_buffer, p->pBuffer + 6, p->size - 6);
                        oversize_offset = p.size - 6;
                        Console.WriteLine(
                            $"First fragment of oversized of seq {seq}: now at {oversize_offset}/{oversize_length}");
                    }
                }
                    break;
                case OpCodes.OP_KeepAlive:
                    break;
                case OpCodes.OP_Ack:
                    //Console.WriteLine($"OP_Ack: {p.pBuffer.NetU16(0)}");
                    break;
                //    default:
                //        {
                //            var ap = MakeApplicationPacket(p);

                //            if (null != ap)
                //            {
                //                InboundQueue.TryAdd(ap);
                //            }
                //        }
                //        break;
            }
        }
    }
}