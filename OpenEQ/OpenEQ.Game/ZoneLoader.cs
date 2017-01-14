using System.Threading.Tasks;
using SiliconStudio.Xenko.Engine;
using SiliconStudio.Core.IO;
using static System.Console;
using OpenEQ.FileConverter;
using OpenEQ.FileConverter.Extensions;
using OpenEQ.FileConverter.Entities;
using System.Collections.Generic;
using OpenEQ.FileConverter.Wld;

namespace OpenEQ
{
    public class ZoneLoader : AsyncScript
    {
        public string ZoneName;
        public string EverquestPath = @"D:\ssdbackup\equf";

        public override async Task Execute()
        {
            var task = new Task(async () => {
                var ofn = $"/cache/{ZoneName}.zip";
                if(!await VirtualFileSystem.FileExistsAsync(ofn)) {
                    WriteLine($"Attempting to convert zone {ZoneName} for first use.");

                    var taskS3dObjFiles = S3DConverter.ReadS3DAsync($@"{EverquestPath}\{ZoneName}_obj.s3d");
                    var taskS3dFiles = S3DConverter.ReadS3DAsync($@"{EverquestPath}\{ZoneName}.s3d");

                    Task.WaitAll(taskS3dObjFiles, taskS3dFiles);
                    var s3dObjFilesDict = taskS3dObjFiles.Result.Merge(taskS3dFiles.Result);
                    var s3dFilesDict = taskS3dFiles.Result.Merge(taskS3dObjFiles.Result);

                    var zone = new Zone();

                    ConvertObjects(s3dObjFilesDict, $"{ZoneName}_obj.wld", zone);
                    ConvertObjects(s3dFilesDict, "objects.wld", zone);
                    ConvertLights(s3dFilesDict, "lights.wld", zone);
                    ConvertZone(s3dFilesDict, $"{ZoneName}.wld", zone);

                    using(var stream = VirtualFileSystem.OpenStream(ofn, VirtualFileMode.Create, VirtualFileAccess.Write))
                        zone.Output(stream);
                }
                WriteLine("Loading zone");
                var rstream = VirtualFileSystem.OpenStream(ofn, VirtualFileMode.Open, VirtualFileAccess.Read);
                var zoneEntity = OEQZoneReader.Read((Game)Game, ZoneName, rstream);
                Entity.AddChild(zoneEntity);
            });
            task.Start();
            await task;
        }

        private static void ConvertZone(IDictionary<string, byte[]> input, string fileName, Zone zone) {
            var wld = new WldConverter();

            if(!input.ContainsKey(fileName)) {
                WriteLine($"Could not find {fileName}.  Skipping.");
                return;
            }

            wld.Convert(input[fileName], input);
            wld.ConvertZone(zone);
        }
        private static void ConvertObjects(IDictionary<string, byte[]> input, string fileName, Zone zone) {
            var wld = new WldConverter();

            if(!input.ContainsKey(fileName)) {
                WriteLine($"Could not find {fileName}.  Skipping.");
                return;
            }

            wld.Convert(input[fileName], input);
            wld.ConvertObjects(zone);
        }

        private static void ConvertLights(IDictionary<string, byte[]> input, string fileName, Zone zone) {
            var wld = new WldConverter();

            if(!input.ContainsKey(fileName)) {
                WriteLine($"Could not find {fileName}.  Skipping.");
                return;
            }

            wld.Convert(input[fileName], input);
            wld.ConvertLights(zone);
        }
    }
}
