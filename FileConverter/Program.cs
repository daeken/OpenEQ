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
            var path = "";
            var zoneName = "";

            if (args.Length < 1)
            {
                Console.WriteLine($"Usage: {System.AppDomain.CurrentDomain.FriendlyName} [Path to EQ] [zonename]");
                Console.WriteLine("Examples:");
                Console.WriteLine(" - To convert all zones");
                Console.WriteLine($@"    {System.AppDomain.CurrentDomain.FriendlyName} E:\Games\EQRoF2\");
                Console.WriteLine();
                Console.WriteLine(" - To convert only the Plane of Mischief");
                Console.WriteLine($@"    {System.AppDomain.CurrentDomain.FriendlyName} E:\Games\EQRoF2\ mischiefplane");
                return;
            }

            // Get the path.
            path = $"{args[0]}\\";

            if (args.Length > 1)
            {
                zoneName = args[1];
            }

            ConvertFiles(path, zoneName);
        }

        private static IEnumerable<string> GetZoneNames(IEnumerable<FileInfo> fi)
        {
            return from f in fi where !f.Name.Contains("_") select f.Name.ToLower().Replace(".s3d", "");
        }

        private static void ConvertFiles(string path, string fileName = null)
        {
            IList<string> zoneNames;

            if (string.IsNullOrEmpty(fileName))
            {
                var di = new DirectoryInfo(path);
                var s3ds = di.GetFiles("*.s3d");

                // Get just the list of name.s3d, ignoring any _obj _chr, etc.
                zoneNames = GetZoneNames(s3ds).ToList();
            }
            else
            {
                zoneNames = new[] {fileName};
            }

            Parallel.ForEach(zoneNames, (file) =>
            {
                Console.WriteLine($"Converting {file}");

                ConvertS3Ds(path, file);

                // TODO: Finish ConvertChr.
                //ConvertChr(path, file);
            });

            Console.WriteLine("Press enter to quit.");
            Console.ReadLine();
        }

        private static async void ConvertChr(string path, string file)
        {
            var chrFiles = await S3DConverter.ReadS3DAsync($@"{path}{file}_chr.s3d");

            var zone = new Zone();

            ConvertCharacters(chrFiles, path, file, zone);
        }

        private static void ConvertS3Ds(string path, string file)
        {
            var taskS3dObjFiles = S3DConverter.ReadS3DAsync($@"{path}{file}_obj.s3d");
            var taskS3dFiles = S3DConverter.ReadS3DAsync($@"{path}{file}.s3d");

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

        private static void ConvertCharacters(IDictionary<string, byte[]> input, string path, string fileName, Zone zone)
        {
            var wld = new WldConverter();

            wld.Convert(input[$"{fileName}_chr.wld"], input);
            wld.ConvertCharacters($"{path}{fileName}_chr.zip", zone);
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