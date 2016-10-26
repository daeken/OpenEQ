using static System.Console;
using System.Linq;
using System;

namespace OpenEQ {
    public static class Utility {
        const string printable = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ-=_+{}[];':\" |\\<>?,./`~!@#$%^&*()1234567890";
        public static void Hexdump(byte[] data) {
            for(var i = 0; i < data.Length; i += 16) {
                Write($"{i:X04}  ");
                var chars = "";
                for(var j = 0; j < 16; ++j) {
                    if(i + j < data.Length) {
                        Write($"{data[i + j]:X02} ");
                        if(printable.Contains((char)data[i + j]))
                            chars += (char)data[i + j];
                        else
                            chars += ".";
                    } else
                        Write("   ");
                    if(j == 7) {
                        Write(" ");
                        chars += " ";
                    }
                }
                Write("| ");
                WriteLine(chars);
            }
            WriteLine($"{data.Length:X04}");
        }

        public static ushort NetU16(this byte[] data, int off) {
            return (ushort)(
                (data[off] << 8) |
                data[off + 1]
            );
        }

        public static T[] Sub<T>(this T[] arr, int start, int end=-1) {
            end = end == -1 ? arr.Length : end;
            var narr = new T[end - start];
            Array.Copy(arr, start, narr, 0, end - start);
            return narr;
        }
    }
}
