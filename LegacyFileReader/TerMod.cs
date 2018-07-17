using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using MoreLinq.Extensions;
using OpenEQ.Common;
using static System.Console;

namespace OpenEQ.LegacyFileReader {
	public class TerMod {
		public readonly bool IsTer;
		public readonly Dictionary<uint, (string Name, string Shader, Dictionary<string, object> Properties)> Materials;
		public readonly List<float> VertexBuffer;
		public readonly Dictionary<(uint MatIndex, bool Collidable), List<uint>> Meshes;
		
		public TerMod(Stream fp, bool isTer) {
			IsTer = isTer;
			var br = new BinaryReader(fp, Encoding.Default, leaveOpen: true);

			var magic = br.ReadUInt32();
			if(isTer)
				Debug.Assert(magic == 0x54475145); // 'EQGT'
			else
				Debug.Assert(magic == 0x4d475145); // 'EQGM'
			
			var version = br.ReadUInt32();
			var strlen = br.ReadInt32();
			var numMat = br.ReadInt32();
			var numVert = br.ReadInt32();
			var numTri = br.ReadInt32();

			if(!isTer)
				br.ReadUInt32(); // Unk
			
			var strTable = Encoding.ASCII.GetString(br.ReadBytes(strlen));
			string GetString(int offset) => strTable.Substring(offset, strTable.IndexOf('\0', offset) - offset);

			Materials = Enumerable.Range(0, numMat).Select(x => {
				var index = br.ReadUInt32();
				var matName = GetString(br.ReadInt32());
				var shader = GetString(br.ReadInt32());
				var numProp = br.ReadInt32();
				return (index, (Name: matName, Shader: shader,
					Properties: Enumerable.Range(0, numProp).Select(_ => {
						var name = GetString(br.ReadInt32());
						object value;
						switch(br.ReadUInt32()) {
							case 0: value = br.ReadSingle(); break;
							case 2: value = GetString(br.ReadInt32()); break;
							case 3: value = br.ReadUInt32(); break;
							case uint v: throw new NotImplementedException($"Unknown .ter material property type {v}");
						}
						return (name, value);
					}).ToDictionary()));
			}).ToDictionary();
			
			if(isTer)
				Materials.Values.ForEach(mat => WriteLine($"{mat.Name} {mat.Shader} ({string.Join(", ", mat.Properties.Select(x => $"{x.Key}={x.Value}"))})"));
			
			VertexBuffer = Enumerable.Range(0, numVert).Select(x => {
				var v = br.ReadVec3().AsArray().Concat(br.ReadVec3().AsArray());
				if(version == 3)
					br.ReadVec3(); // Unk
				return v.Concat(br.ReadVec2().AsArray());
			}).SelectMany(x => x).ToList();

			var polygons = Enumerable.Range(0, numTri).Select(x => (A: br.ReadUInt32(), B: br.ReadUInt32(),
				C: br.ReadUInt32(), MatId: br.ReadUInt32(), Flags: br.ReadUInt32()));
			
			Meshes = new Dictionary<(uint, bool), List<uint>>();
			polygons.ForEach(poly => {
				var key = (poly.MatId, true);
				if(!Meshes.ContainsKey(key))
					Meshes[key] = new List<uint>();
				var m = Meshes[key];
				m.Add(poly.A);
				m.Add(poly.B);
				m.Add(poly.C);
			});
		}
	}
}