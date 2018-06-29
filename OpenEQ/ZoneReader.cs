using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using static System.Console;

namespace OpenEQ {
	public struct ZoneMesh {
		public uint MatId;
		public bool Collidable;
		public float[] Positions, Normals, TexCoords;
		public uint[] Indices;
	}
	
	public class ZoneReader {
		public readonly List<(uint Flags, uint Param, List<Stream> Textures)> Materials;
		public readonly List<List<ZoneMesh>> Objects;

		readonly ZipArchive Zip;
		
		public ZoneReader(string fn) {
			Zip = ZipFile.Open(fn, ZipArchiveMode.Read);
			using(var zf = Zip.GetEntry("zone.oez")?.Open()) {
				using(var br = new BinaryReader(zf)) {
					Materials = Enumerable.Range(0, br.ReadInt32()).Select(i =>
						(br.ReadUInt32(), br.ReadUInt32(), 
							Enumerable.Range(0, br.ReadInt32()).Select(j => Zip.GetEntry(br.ReadString())?.Open()).ToList()
						) 
					).ToList();
					Objects = Enumerable.Range(0, br.ReadInt32()).Select(i =>
						Enumerable.Range(0, br.ReadInt32()).Select(j => {
							var matId = br.ReadUInt32();
							var collidable = br.ReadUInt32() != 0U;
							var vcount = br.ReadInt32();
							var positions = Enumerable.Range(0, vcount * 3).Select(_ => br.ReadSingle()).ToArray();
							var normals = Enumerable.Range(0, vcount * 3).Select(_ => br.ReadSingle()).ToArray();
							var texcoords = Enumerable.Range(0, vcount * 2).Select(_ => br.ReadSingle()).ToArray();
							var indices = Enumerable.Range(0, br.ReadInt32() * 3).Select(_ => br.ReadUInt32()).ToArray();
							return new ZoneMesh {
								MatId = matId, 
								Collidable = collidable, 
								Positions = positions, 
								Normals = normals, 
								TexCoords = texcoords, 
								Indices = indices
							};
						}).ToList()
					).ToList();
				}
			}
		}
	}
}