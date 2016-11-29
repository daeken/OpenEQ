using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenEQ.FileConverter.Extensions
{
    public static class StringExtensions
    {
        /// <summary>
        /// Read a null terminated string within a string.
        /// </summary>
        /// <param name="input"></param>
        /// <param name="offset"></param>
        /// <returns></returns>
        public static string ReadNullTerminatedString(this string input, int offset)
        {
            var startPos = offset;
            while (offset < input.Length)
            {
                if ('\0' != input[offset])
                {
                    offset++;
                }
                else
                {
                    // Offset - 1 because I don't want the null terminator.
                    return input.Substring(startPos, offset - startPos);
                }
            }

            return "";
        }

        private static readonly byte[] XorKey = { 0x95, 0x3A, 0xC5, 0x2A, 0x95, 0x7A, 0x95, 0x6A };

        public static string DecodeString(this byte[] hash)
        {
            for (var i = 0; i < hash.Length; i++)
            {
                hash[i] ^= XorKey[i % XorKey.Length];
            }

            return Encoding.UTF8.GetString(hash);
        }
    }
}
