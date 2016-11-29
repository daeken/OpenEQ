
using System;
using System.Collections;
using System.Collections.Generic;

namespace OpenEQ.FileConverter.Extensions
{
    using System.IO;

    public static class BinaryReaderExtensions
    {
        /// <summary>
        /// BinaryReader's ReadString method is designed to read strings that were written by BinaryReader.  Meaning,
        /// the string length will be exactly formatted the way it wants.  The format we're parsing isn't quite a match
        /// so we can't use the default ReadString implementation.
        /// </summary>
        /// <param name="reader"></param>
        /// <param name="length"></param>
        /// <param name="trimNullTerminator"></param>
        /// <returns></returns>
        public static string ReadString(this BinaryReader reader, int length, bool trimNullTerminator = true)
        {
            var tmp = string.Join("", reader.ReadChars(length));

            return trimNullTerminator ? tmp.Trim(char.MinValue) : tmp;
        }

        public static ushort[] ReadUInt16(this BinaryReader reader, int length)
        {
            var list = new ushort[length];
            for (var i = 0; i < length; i++)
            {
                list[i] = reader.ReadUInt16();
            }
            return list;
        }

        public static int[] ReadInt32(this BinaryReader reader, uint length)
        {
            var list = new int[length];
            for (var i = 0; i < length; i++)
            {
                list[i] = reader.ReadInt32();
            }
            return list;
        }

        public static uint[] ReadUInt32(this BinaryReader reader, uint length)
        {
            var list = new uint[length];
            for (var i = 0; i < length; i++)
            {
                list[i] = reader.ReadUInt32();
            }
            return list;
        }

        public static float[] ReadSingle(this BinaryReader reader, uint length)
        {
            var list = new float[length];
            for (var i = 0; i < length; i++)
            {
                list[i] = reader.ReadSingle();
            }
            return list;
        }
    }
}