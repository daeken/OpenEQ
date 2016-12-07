using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using PacketRipper.Extensions;

namespace PacketRipper
{
    class Program
    {
        static void Main(string[] args)
        {
            //var a =
            //    "90-7D-65-71-63-68-61-74-2E-65-71-65-6D-75-6C-61-74-6F-72-2E-6E-65-74-2C-37-35-30-30-2C-49-6D-70-65-72-69-75-6D-2E-42-69-67-64-75-64-65-66-6F-75-72-2C-55-37-34-31-30-33-38-34-43-00"
            //        .StringToByteArray();
            //var b = Encoding.UTF8.GetString(a);

            var fileName = "UF_Login_CharCreate_TutorialB.csv";
            var dedup = new Dictionary<int, bool>();

            using (var sr = new StreamReader($@"E:\Repos\OpenEQ\Tools\{fileName}"))
            {
                string currentLine;
                var lineNum = 0;
                // currentLine will be null when the StreamReader reaches the end of file
                while ((currentLine = sr.ReadLine()) != null)
                {
                    // Packet dumps can have dupes, so strip them.
                    var key = currentLine.GetHashCode();
                    if (dedup.ContainsKey(key)) continue;
                    dedup.Add(key, true);

                    var fields = currentLine.Replace("-", "").Split(new [] { ',' }, StringSplitOptions.RemoveEmptyEntries);
                    PacketFactory.Instance.Process(fields);
                    //Console.WriteLine();
                }
            }
        }
    }
}
