using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using OpenEQ.FileConverter.Entities;
using OpenEQ.FileConverter.Extensions;
using OpenEQ.FileConverter.Wld;

namespace OpenEQ.FileConverter
{
    class Program
    {
        static void Main(string[] args)
        {
            //if (args.Length < 1)
            //{
            //    Console.WriteLine($"Usage: {System.AppDomain.CurrentDomain.FriendlyName} [Path to EQ] [zonename]");
            //    Console.WriteLine("Examples:");
            //    Console.WriteLine(" - To convert all zones");
            //    Console.WriteLine($@"    {System.AppDomain.CurrentDomain.FriendlyName} E:\Games\EQRoF2\");
            //    Console.WriteLine();
            //    Console.WriteLine(" - To convert only the Plane of Mischief");
            //    Console.WriteLine($@"    {System.AppDomain.CurrentDomain.FriendlyName} E:\Games\EQRoF2\ mischiefplane");
            //    return;
            //}
            //TODO: Read from config or command line or something.
            //var fileName = "mischiefplane";
            var path = @"E:\Games\EQRoF2\";
            var fileName = "";
            //fileName = "gfaydark";
            ConvertS3Ds(path, fileName);
        }

        private static IEnumerable<string> GetZoneNames(IEnumerable<FileInfo> fi)
        {
            return from f in fi where !f.Name.Contains("_") select f.Name.ToLower().Replace(".s3d", "");
        }

        private static void ConvertS3Ds(string path, string fileName = null)
        {
            IList<string> zoneNames;

            if (string.IsNullOrEmpty(fileName))
            {
                var di = new DirectoryInfo(@"E:\Games\EQRoF2\");
                var s3ds = di.GetFiles("*.s3d");

                // Get just the list of name.s3d, ignoring any _obj _chr, etc.
                zoneNames = GetZoneNames(s3ds).ToList();
            }
            else
            {
                zoneNames = new[] { fileName };
            }

            var sw = Stopwatch.StartNew();
            Parallel.ForEach(zoneNames, (file) =>
            {
                Console.WriteLine($"Converting {file}");

                var taskS3dObjFiles = S3DConverter.ConvertAsync($@"{path}{file}_obj.s3d");
                var taskS3dFiles = S3DConverter.ConvertAsync($@"{path}{file}.s3d");

                // We need the S3D to be fully parsed before we can continue.
                Task.WaitAll(taskS3dObjFiles, taskS3dFiles);

                // Left merge both so that we keep values in dictionary A if they exist in A and B.
                // Verified behavior with a smaller set of dictionaries.
                var s3dObjFilesDict = taskS3dObjFiles.Result.Merge(taskS3dFiles.Result);
                var s3dFilesDict = taskS3dFiles.Result.Merge(taskS3dObjFiles.Result);

                var zone = new Zone();

                ConvertObjects(s3dObjFilesDict, $"{file}_obj.wld", zone);
                ConvertObjects(s3dFilesDict, "objects.wld", zone);
                ConvertLights(s3dFilesDict, "lights.wld", zone);
                ConvertZone(s3dFilesDict, $"{file}.wld", zone);

                zone.Output($@"{path}{file}.zip");
            });

            sw.Stop();
            Console.WriteLine($"Elapsed Time: {sw.ElapsedMilliseconds}");
            Console.WriteLine("Press enter to quit.");
            Console.ReadLine();

        }

        private static void ConvertObjects(IDictionary<string, byte[]> input, string fileName, Zone zone)
        {
            var wld = new WldConverter();

            if (!input.ContainsKey(fileName))
            {
                Console.WriteLine($"Could not find {fileName}.  Skipping.");
                return;
            }

            wld.Convert(input[fileName], input);
            wld.ConvertObjects(zone);
        }

        private static void ConvertLights(IDictionary<string, byte[]> input, string fileName, Zone zone)
        {
            var wld = new WldConverter();

            if (!input.ContainsKey(fileName))
            {
                Console.WriteLine($"Could not find {fileName}.  Skipping.");
                return;
            }

            wld.Convert(input[fileName], input);
            wld.ConvertLights(zone);
        }

        private static void ConvertZone(IDictionary<string, byte[]> input, string fileName, Zone zone)
        {
            var wld = new WldConverter();

            if (!input.ContainsKey(fileName))
            {
                Console.WriteLine($"Could not find {fileName}.  Skipping.");
                return;
            }

            wld.Convert(input[fileName], input);
            wld.ConvertZone(zone);
        }
    }
}