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
            var sw = Stopwatch.StartNew();
            //TODO: Read from config or command line or something.
            var fileName = "gfaydark";
            var path = @"E:\Games\EQRoF2\";

            var taskS3dObjFiles = S3DConverter.Convert($@"{path}{fileName}_obj.s3d");

            var taskS3dFiles = S3DConverter.Convert($@"{path}{fileName}.s3d");

            // We need the S3D to be fully parsed before we can continue.
            Task.WaitAll(taskS3dObjFiles, taskS3dFiles);

            // Left merge both so that we keep values in dictionary A if they exist in A and B.
            var s3dObjFilesDict = taskS3dObjFiles.Result.Merge(taskS3dFiles.Result);
            var s3dFilesDict = taskS3dFiles.Result.Merge(taskS3dObjFiles.Result);

            var zone = new Zone();

            // Leaving this for now even though it's not a huge benefit as everything is actually
            // running in sequence.  There's a race condition inside convert that relates to Placeable
            // objects.  I'll work that one out so I can reconcile the list at the end and make these
            // async again.

            ConvertObjects(s3dObjFilesDict, $"{fileName}_obj.wld", zone);
            ConvertObjects(s3dFilesDict, "objects.wld", zone);
            ConvertLights(s3dFilesDict, "lights.wld", zone);
            ConvertZone(s3dFilesDict, $"{fileName}.wld", zone);

            zone.Output($@"{path}{fileName}.zip");

            sw.Stop();

            var a = 1;
        }

        private static void ConvertObjects(IDictionary<string, byte[]> input, string fileName, Zone zone)
        {
            var wld = new WldConverter();
            wld.Convert(input[fileName], input);
            wld.ConvertObjects(zone);
        }

        private static void ConvertLights(IDictionary<string, byte[]> input, string fileName, Zone zone)
        {
            var wld = new WldConverter();
            wld.Convert(input[fileName], input);
            wld.ConvertLights(zone);
        }

        private static void ConvertZone(IDictionary<string, byte[]> input, string fileName, Zone zone)
        {
            var wld = new WldConverter();
            wld.Convert(input[fileName], input);
            wld.ConvertZone(zone);
        }
    }
}