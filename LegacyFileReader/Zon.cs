using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Numerics;
using System.Text;
using MoreLinq;
using OpenEQ.Common;
using static System.Console;

namespace OpenEQ.LegacyFileReader {
	public class Zon {
		public readonly S3D S3D;

		public readonly List<TerMod> Objects;
		public readonly List<(int ObjId, string Name, Vector3 Position, Vector3 Rotation, float Scale)> Placeables;
		public readonly List<(string Name, Vector3 Position, Vector3 Color, float Radius)> Lights;
		
		public Zon(S3D s3d, Stream fp) {
			S3D = s3d;
			var br = new BinaryReader(fp, Encoding.Default, leaveOpen: true);

			var magic = br.ReadUInt32();
			Debug.Assert(magic == 0x5a475145); // 'EQGZ'
			var version = br.ReadUInt32();
			Debug.Assert(version == 1);
			var strlen = br.ReadInt32();
			var numFiles = br.ReadInt32();
			var numPlaceable = br.ReadInt32();
			var numUnk = br.ReadInt32();
			var numLights = br.ReadInt32();

			var strTable = Encoding.ASCII.GetString(br.ReadBytes(strlen));
			string GetString(int offset) => strTable.Substring(offset, strTable.IndexOf('\0', offset) - offset);

			Objects = Enumerable.Range(0, numFiles).Select(x => {
				var fn = GetString(br.ReadInt32()).ToLower();
				if(fn.EndsWith(".ter"))
					return new TerMod(s3d.Open(fn), true);
				if(fn.EndsWith(".mod"))
					return new TerMod(s3d.Open(fn), false);
				throw new NotImplementedException($"Unknown file reference in .zon: {fn}");
			}).ToList();

			Placeables = Enumerable.Range(0, numPlaceable).Select(x => {
				var objId = br.ReadInt32();
				var name = GetString(br.ReadInt32());
				var pos = br.ReadVec3();
				var rot = br.ReadVec3();
				var scale = br.ReadSingle();
				return (objId, name, pos, rot, scale);
			}).ToList();

			Enumerable.Range(0, numUnk).ForEach(x => {
				br.ReadUInt32();
				br.ReadVec3();
				br.ReadVec3();
				br.ReadVec3();
			});

			var lt = new Vector3(1, -1, 1);
			Lights = Enumerable.Range(0, numLights)
				.Select(x => (GetString(br.ReadInt32()), br.ReadVec3().YXZ() * lt, br.ReadVec3(), br.ReadSingle())).ToList();
		}
	}
}