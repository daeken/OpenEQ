
namespace PacketRipper.Extensions
{
    using System;
    using System.Linq;

    public static class StringExtensions
    {
        public static byte[] StringToByteArray(this string hex)
        {
            hex = hex.Replace("-", "");
            return Enumerable.Range(0, hex.Length)
                .Where(x => x%2 == 0)
                .Select(x => Convert.ToByte(hex.Substring(x, 2), 16))
                .ToArray();
        }
    }
}