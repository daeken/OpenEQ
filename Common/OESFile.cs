using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Numerics;
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

		public IEnumerable<T> Find<T>() where T : OESChunk => Children.OfType<T>();
		public IEnumerable<OESChunk> Find<T1, T2>() where T1 : OESChunk where T2 : OESChunk =>
			Children.Where(x => x is T1 || x is T2);
		public IEnumerable<OESChunk> Find<T1, T2, T3>() where T1 : OESChunk where T2 : OESChunk where T3 : OESChunk =>
			Children.Where(x => x is T1 || x is T2 || x is T3);

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
			bw.Write((uint) Children.Count);
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
			AlphaMask = br.ReadBool();
			Transparent = br.ReadBool();
			Emissive = br.ReadBool();
		}
	}

	public class OESEffect : OESChunk {
		public string Name;
		public readonly Dictionary<string, object> Parameters = new Dictionary<string, object>();
		
		public OESEffect(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESEffect(string name) : base("fx") => Name = name;

		public object this[string name] {
			get => Parameters[name];
			set => Parameters[name] = value;
		}

		public T Get<T>(string name) => (T) Parameters[name];
		
		protected override void SerializeData(BinaryWriter bw) {
			bw.WriteUTF8String(Name);
			bw.Write(Parameters.Count);

			foreach(var (name, value) in Parameters) {
				bw.WriteUTF8String(name);
				switch(value) {
					case uint uv:
						bw.Write(0U);
						bw.Write(uv);
						break;
					case float fv:
						bw.Write(1U);
						bw.Write(fv);
						break;
					case Vector2 v2v:
						bw.Write(2U);
						bw.Write(v2v);
						break;
					case Vector3 v3v:
						bw.Write(3U);
						bw.Write(v3v);
						break;
					case Vector4 v4v:
						bw.Write(4U);
						bw.Write(v4v);
						break;
					case Matrix4x4 m4v:
						bw.Write(16U);
						m4v.AsArray().ForEach(bw.Write);
						break;
					case string sv:
						bw.Write(64);
						bw.WriteUTF8String(sv);
						break;
					default:
						throw new NotImplementedException();
				}
			}
		}

		protected override void DeserializeData(BinaryReader br) {
			Name = br.ReadUTF8String();
			var numParams = br.ReadUInt32();
			for(var i = 0; i < numParams; ++i) {
				var name = br.ReadUTF8String();
				object value;
				switch(br.ReadUInt32()) {
					case 0: value = br.ReadUInt32(); break;
					case 1: value = br.ReadSingle(); break;
					case 2: value = br.ReadVec2(); break;
					case 3: value = br.ReadVec3(); break;
					case 4: value = br.ReadVec4(); break;
					case 16: throw new NotImplementedException(); // TODO
					case 64: value = br.ReadUTF8String(); break;
					default: throw new NotImplementedException();
				}
				Parameters[name] = value;
			}
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
		public Vector3 Position, Scale;
		public Quaternion Rotation;
		public string SkinName;

		public OESInstance(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESInstance(OESObject obj, Vector3 position, Vector3 scale, Quaternion rotation, string skinName = "") : base("inst") {
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
			Rotation = br.ReadQuaternion();
		}
	}

	public class OESLight : OESChunk {
		public Vector3 Position, Color;
		public float Radius, Attenuation;

		public OESLight(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESLight(Vector3 position, Vector3 color, float radius, float attenuation) : base("lit") {
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

		protected override void SerializeData(BinaryWriter bw) => bw.WriteUTF8String(Name ?? "");
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
		public static T Read<T>(Stream fp) where T : OESChunk => (T) Read(fp);
		
		public static OESChunk Read(Stream fp) {
			using(var br = new BinaryReader(fp))
				return new OESFile().Read(br);
		}

		public static void Write(Stream fp, OESChunk chunk) {
			using(var bw = new BinaryWriter(fp))
				chunk.Serialize(bw);
		}
		
		readonly Dictionary<uint, List<Action<OESChunk>>> Resolvers = new Dictionary<uint, List<Action<OESChunk>>>();
		readonly Dictionary<uint, OESChunk> Chunks = new Dictionary<uint, OESChunk>();

		OESChunk Read(BinaryReader br) {
			var instance = OESChunk.Deserialize(this, br);
			foreach(var (id, funcs) in Resolvers)
				funcs.ForEach(x => x(Chunks[id]));
			return instance;
		}

		internal void Add(OESChunk chunk) => Chunks[chunk.Id] = chunk;

		internal void Resolve<T>(uint id, Action<T> func) where T : OESChunk {
			if(!Resolvers.ContainsKey(id)) Resolvers[id] = new List<Action<OESChunk>>();
			Resolvers[id].Add(chunk => {
				Debug.Assert(chunk is T);
				func((T) chunk);
			});
		}
	}
}