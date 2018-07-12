using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using MoreLinq;

namespace OpenEQ.Common {
	public class OESChunk : IEnumerable<OESChunk> {
		static uint IdGen;

		OESFile OESFile;
		
		public readonly string TypeCode;
		public readonly uint Id;
		public readonly List<OESChunk> Children = new List<OESChunk>();

		public OESChunk(string typeCode) {
			TypeCode = typeCode;
			Id = ++IdGen;
		}

		protected OESChunk(string typeCode, uint id) {
			TypeCode = typeCode;
			Id = id;
		}

		public void Add(OESChunk child) => Children.Add(child);
		public IEnumerator<OESChunk> GetEnumerator() => Children.GetEnumerator();
		IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();

		internal void Serialize(BinaryWriter bw) {
			Debug.Assert(TypeCode.Length <= 4);
			bw.Write(Encoding.ASCII.GetBytes(TypeCode + "    ".Substring(TypeCode.Length)));
			bw.Write(Id);

			byte[] data;
			using(var ms = new MemoryStream())
				using(var mbw = new BinaryWriter(ms)) {
					SerializeData(mbw);
					mbw.Flush();
					data = ms.ToArray();
				}
			byte[] childData;
			using(var ms = new MemoryStream())
				using(var mbw = new BinaryWriter(ms)) {
					foreach(var child in Children)
						child.Serialize(mbw);
					mbw.Flush();
					childData = ms.ToArray();
				}
			
			bw.Write((uint) data.LongLength);
			bw.Write((uint) childData.LongLength);
			bw.Write(data);
			bw.Write(childData);
		}

		protected virtual void SerializeData(BinaryWriter bw) { }
		protected virtual void DeserializeData(BinaryReader br) { }

		internal static OESChunk Deserialize(OESFile file, BinaryReader br) {
			var tc = Encoding.ASCII.GetString(br.ReadBytes(4)).TrimEnd(' ');
			var id = br.ReadUInt32();
			var dlen = br.ReadUInt32();
			var cdlen = br.ReadUInt32();
			var numChildren = br.ReadUInt32();
			var epos = br.BaseStream.Position + dlen;

			OESChunk instance;
			switch(tc) {
				case "mat": instance = new OESMaterial(tc, id); break;
				case "fx": instance = new OESEffect(tc, id); break;
				case "tex": instance = new OESTexture(tc, id); break;
				case "zone": instance = new OESZone(tc, id); break;
				case "regn": instance = new OESRegion(tc, id); break;
				case "char": instance = new OESCharacter(tc, id); break;
				case "obj": instance = new OESObject(tc, id); break;
				case "inst": instance = new OESInstance(tc, id); break;
				case "lit": instance = new OESLight(tc, id); break;
				case "skin": instance = new OESSkin(tc, id); break;
				case "mesh": instance = new OESStaticMesh(tc, id); break;
				default: instance = new OESChunk(tc, id); break;
			}
			file.Add(instance);
			instance.OESFile = file;
			instance.DeserializeData(br);
			Debug.Assert(br.BaseStream.Position <= epos);
			br.BaseStream.Position = epos;

			for(var i = 0; i < numChildren; ++i)
				instance.Add(Deserialize(file, br));
			
			Debug.Assert(br.BaseStream.Position == epos + cdlen);

			return instance;
		}

		protected void Resolve<T>(uint id, Action<T> func) where T : OESChunk => OESFile.Resolve(id, func);
	}

	public class OESMaterial : OESChunk {
		public bool AlphaMask, Transparent, Emissive;
		
		public OESMaterial(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESMaterial(bool alphaMask, bool transparent, bool emissive) : base("mat") {
			AlphaMask = alphaMask;
			Transparent = transparent;
			Emissive = emissive;
		}

		protected override void SerializeData(BinaryWriter bw) {
			bw.WriteBool(AlphaMask);
			bw.WriteBool(Transparent);
			bw.WriteBool(Emissive);
		}

		protected override void DeserializeData(BinaryReader br) {
			AlphaMask = br.ReadBoolean();
			Transparent = br.ReadBoolean();
			Emissive = br.ReadBoolean();
		}
	}

	public class OESEffect : OESChunk {
		public string Name;

		public OESEffect(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESEffect(string name) : base("fx") => Name = name;
		
		protected override void SerializeData(BinaryWriter bw) {
			bw.WriteUTF8String(Name);
			bw.Write(0U);
		}

		protected override void DeserializeData(BinaryReader br) {
			Name = br.ReadUTF8String();
			var numParams = br.ReadUInt32();
			Debug.Assert(numParams == 0);
		}
	}

	public class OESTexture : OESChunk {
		public string Filename;

		public OESTexture(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESTexture(string filename) : base("tex") => Filename = filename;

		protected override void SerializeData(BinaryWriter bw) => bw.WriteUTF8String(Filename);
		protected override void DeserializeData(BinaryReader br) => Filename = br.ReadUTF8String();
	}

	public class OESZone : OESChunk {
		public string Name;

		public OESZone(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESZone(string name) : base("zone") => Name = name;

		protected override void SerializeData(BinaryWriter bw) => bw.WriteUTF8String(Name);
		protected override void DeserializeData(BinaryReader br) => Name = br.ReadUTF8String();
	}

	public class OESRegion : OESChunk {
		public OESRegion(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESRegion() : base("regn") {}
		
		protected override void SerializeData(BinaryWriter bw) => throw new System.NotImplementedException();
	}

	public class OESCharacter : OESChunk {
		public OESCharacter(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESCharacter() : base("char") {}
		
		protected override void SerializeData(BinaryWriter bw) => throw new System.NotImplementedException();
	}

	public class OESObject : OESChunk {
		public string Name;

		public OESObject(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESObject(string name = "") : base("obj") => Name = name;

		protected override void SerializeData(BinaryWriter bw) => bw.WriteUTF8String(Name ?? "");
		protected override void DeserializeData(BinaryReader br) => Name = br.ReadUTF8String();
	}

	public class OESInstance : OESChunk {
		public OESObject Object;
		public Vec3 Position, Scale;
		public Vec4 Rotation;
		public string SkinName;

		public OESInstance(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESInstance(OESObject obj, Vec3 position, Vec3 scale, Vec4 rotation, string skinName = "") : base("inst") {
			Object = obj;
			Position = position;
			Scale = scale;
			Rotation = rotation;
			SkinName = skinName;
		}

		protected override void SerializeData(BinaryWriter bw) {
			bw.Write(Object.Id);
			bw.WriteUTF8String(SkinName ?? "");
			bw.Write(Position);
			bw.Write(Scale);
			bw.Write(Rotation);
		}

		protected override void DeserializeData(BinaryReader br) {
			Resolve<OESObject>(br.ReadUInt32(), v => Object = v);
			SkinName = br.ReadUTF8String();
			Position = br.ReadVec3();
			Scale = br.ReadVec3();
			Rotation = br.ReadVec4();
		}
	}

	public class OESLight : OESChunk {
		public Vec3 Position, Color;
		public float Radius, Attenuation;

		public OESLight(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESLight(Vec3 position, Vec3 color, float radius, float attenuation) : base("lit") {
			Position = position;
			Color = color;
			Radius = radius;
			Attenuation = attenuation;
		}

		protected override void SerializeData(BinaryWriter bw) {
			bw.Write(Position);
			bw.Write(Color);
			bw.Write(Radius);
			bw.Write(Attenuation);
		}

		protected override void DeserializeData(BinaryReader br) {
			Position = br.ReadVec3();
			Color = br.ReadVec3();
			Radius = br.ReadSingle();
			Attenuation = br.ReadSingle();
		}
	}

	public class OESSkin : OESChunk {
		public string Name;

		public OESSkin(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESSkin(string name = "") : base("skin") => Name = name;

		protected override void SerializeData(BinaryWriter bw) => bw.Write(Name ?? "");
		protected override void DeserializeData(BinaryReader br) => Name = br.ReadUTF8String();
	}

	public class OESStaticMesh : OESChunk {
		public bool Collidable;
		public IReadOnlyList<uint> IndexBuffer;
		public IReadOnlyList<float> VertexBuffer;

		public OESStaticMesh(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESStaticMesh(bool collidable, IReadOnlyList<uint> indexBuffer, IReadOnlyList<float> vertexBuffer) : base("mesh") {
			Collidable = collidable;
			IndexBuffer = indexBuffer;
			VertexBuffer = vertexBuffer;
		}

		protected override void SerializeData(BinaryWriter bw) {
			bw.WriteBool(Collidable);
			bw.Write(IndexBuffer.Count);
			bw.Write(VertexBuffer.Count / 8);
			IndexBuffer.ForEach(bw.Write);
			VertexBuffer.ForEach(bw.Write);
		}

		protected override void DeserializeData(BinaryReader br) {
			Collidable = br.ReadBool();
			var ibc = br.ReadInt32();
			var vbc = br.ReadInt32() * 8;
			IndexBuffer = Enumerable.Range(0, ibc).Select(_ => br.ReadUInt32()).ToList();
			VertexBuffer = Enumerable.Range(0, vbc).Select(_ => br.ReadSingle()).ToList();
		}
	}

	public class OESFile {
		public static OESChunk Read(Stream fp) {
			using(var br = new BinaryReader(fp))
				return new OESFile().Read(br);
		}

		public static void Write(Stream fp, OESChunk chunk) {
			using(var bw = new BinaryWriter(fp))
				chunk.Serialize(bw);
		}
		
		readonly Dictionary<uint, Action<OESChunk>> Resolvers = new Dictionary<uint, Action<OESChunk>>();
		readonly Dictionary<uint, OESChunk> Chunks = new Dictionary<uint, OESChunk>();

		OESChunk Read(BinaryReader br) {
			var instance = OESChunk.Deserialize(this, br);
			foreach(var (id, func) in Resolvers)
				func(Chunks[id]);
			return instance;
		}

		internal void Add(OESChunk chunk) => Chunks[chunk.Id] = chunk;

		internal void Resolve<T>(uint id, Action<T> func) where T : OESChunk {
			Resolvers[id] = chunk => {
				Debug.Assert(chunk is T);
				func((T) chunk);
			};
		}
	}
}