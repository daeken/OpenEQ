using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using OpenEQ.Engine;
using static System.Console;

namespace OpenEQ {
	public struct ZoneMesh {
		public int MatId;
		public bool Collidable;
		public float[] VertexBuffer;
		public uint[] Indices;
	}

	public struct Placeable {
		public int ObjId;
		public Vec3 Position, Rotation, Scale;
	}

	public struct ZoneLight {
		public Vec3 Position, Color;
		public float Radius, Attenuation;
		public uint Flags;
	}
	
	public class ZoneReader {
		public readonly List<(uint Flags, uint Param, List<Stream> Textures)> Materials;
		public readonly List<List<ZoneMesh>> Objects;
		public readonly List<Placeable> Placeables;
		public readonly List<ZoneLight> Lights;

		public ZoneReader(string fn) {
			var zip = ZipFile.Open(fn, ZipArchiveMode.Read);
			using(var zf = zip.GetEntry("zone.oez")?.Open()) {
				using(var br = new BinaryReader(zf)) {
					Materials = Enumerable.Range(0, br.ReadInt32()).Select(i =>
						(br.ReadUInt32(), br.ReadUInt32(), 
							Enumerable.Range(0, br.ReadInt32()).Select(j => zip.GetEntry(br.ReadString())?.Open()).ToList()
						) 
					).ToList();
					Objects = Enumerable.Range(0, br.ReadInt32()).Select(i =>
						Enumerable.Range(0, br.ReadInt32()).Select(j => {
							var matId = br.ReadInt32();
							var collidable = br.ReadUInt32() != 0U;
							var buf = Enumerable.Range(0, br.ReadInt32() * (3 + 3 + 2 + 1)).Select(_ => br.ReadSingle()).ToArray();
							var indices = Enumerable.Range(0, br.ReadInt32() * 3).Select(_ => br.ReadUInt32()).ToArray();
							return new ZoneMesh {
								MatId = matId, 
								Collidable = collidable, 
								VertexBuffer = buf, 
								Indices = indices
							};
						}).ToList()
					).ToList();
					Placeables = Enumerable.Range(0, br.ReadInt32()).Select(i =>
						new Placeable {
							ObjId = br.ReadInt32(), 
							Position = new Vec3(br.ReadSingle(), br.ReadSingle(), br.ReadSingle()), 
							Rotation = new Vec3(br.ReadSingle(), br.ReadSingle(), br.ReadSingle()), 
							Scale = new Vec3(br.ReadSingle(), br.ReadSingle(), br.ReadSingle())
						}
					).ToList();
					Lights = Enumerable.Range(0, br.ReadInt32()).Select(i =>
						new ZoneLight {
							Position = new Vec3(br.ReadSingle(), br.ReadSingle(), br.ReadSingle()),
							Color = new Vec3(br.ReadSingle(), br.ReadSingle(), br.ReadSingle()),
							Radius = br.ReadSingle(), 
							Attenuation = br.ReadSingle(), 
							Flags = br.ReadUInt32()
						}
					).ToList();
				}
			}
		}
	}
}