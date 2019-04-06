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
				case "root": instance = new OESRoot(tc, id); break;
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
				case "aset": instance = new OESAnimationSet(tc, id); break;
				case "amsh": instance = new OESAnimatedMesh(tc, id); break;
				case "abuf": instance = new OESAnimationBuffer(tc, id); break;
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

	public class OESRoot : OESChunk {
		public OESRoot(string typeCode, uint id) : base(typeCode, id) {}
		public OESRoot() : base("root") {}

		public override string ToString() => "OESRoot";
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

		public override string ToString() => $"OESMaterial(AlphaMask={AlphaMask}, Transparent={Transparent}, Emissive={Emissive})";
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

		public override string ToString() => $"OESEffect(Name={Name})";
	}

	public class OESTexture : OESChunk {
		public string Filename;

		public OESTexture(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESTexture(string filename) : base("tex") => Filename = filename;

		protected override void SerializeData(BinaryWriter bw) => bw.WriteUTF8String(Filename);
		protected override void DeserializeData(BinaryReader br) => Filename = br.ReadUTF8String();

		public override string ToString() => $"OESTexture(Filename={Filename})";
	}

	public class OESZone : OESChunk {
		public string Name;

		public OESZone(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESZone(string name) : base("zone") => Name = name;

		protected override void SerializeData(BinaryWriter bw) => bw.WriteUTF8String(Name);
		protected override void DeserializeData(BinaryReader br) => Name = br.ReadUTF8String();

		public override string ToString() => $"OESZone(Name={Name})";
	}

	public class OESRegion : OESChunk {
		public OESRegion(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESRegion() : base("regn") {}
		
		protected override void SerializeData(BinaryWriter bw) => throw new NotImplementedException();

		public override string ToString() => "OESRegion";
	}

	public class OESCharacter : OESChunk {
		public string Name;
		
		public OESCharacter(string typeCode, uint id) : base(typeCode, id) {}

		public OESCharacter(string name = "") : base("char") => Name = name;
		
		protected override void SerializeData(BinaryWriter bw) => bw.WriteUTF8String(Name ?? "");
		protected override void DeserializeData(BinaryReader br) => Name = br.ReadUTF8String();

		public override string ToString() => $"OESCharacter(Name={Name})";
	}

	public class OESObject : OESChunk {
		public string Name;

		public OESObject(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESObject(string name = "") : base("obj") => Name = name;

		protected override void SerializeData(BinaryWriter bw) => bw.WriteUTF8String(Name ?? "");
		protected override void DeserializeData(BinaryReader br) => Name = br.ReadUTF8String();

		public override string ToString() => $"OESObject(Name={Name})";
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

		public override string ToString() => $"OESInstance(Object={Object}, Position={Position}, Scale={Scale}, Rotation={Rotation}, SkinName={SkinName})";
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

		public override string ToString() => $"OESLight(Position={Position}, Color={Color}, Radius={Radius}, Attenuation={Attenuation})";
	}

	public class OESSkin : OESChunk {
		public string Name;

		public OESSkin(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESSkin(string name = "") : base("skin") => Name = name;

		protected override void SerializeData(BinaryWriter bw) => bw.WriteUTF8String(Name ?? "");
		protected override void DeserializeData(BinaryReader br) => Name = br.ReadUTF8String();

		public override string ToString() => $"OESSkin(Name={Name})";
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

		public override string ToString() => $"OESStaticMesh(Collidable={Collidable}, IndexBuffer.Count={IndexBuffer.Count}, VertexBuffer.Count={VertexBuffer.Count})";
	}

	public class OESAnimationSet : OESChunk {
		public string Name;
		public float Speed;
		
		public OESAnimationSet(string typeCode, uint id) : base(typeCode, id) {}

		public OESAnimationSet(string name, float speed) : base("aset") {
			Name = name;
			Speed = speed;
		}

		protected override void SerializeData(BinaryWriter bw) {
			bw.WriteUTF8String(Name);
			bw.Write(Speed);
		}

		protected override void DeserializeData(BinaryReader br) {
			Name = br.ReadUTF8String();
			Speed = br.ReadSingle();
		}

		public override string ToString() => $"OESAnimationSet(Name={Name}, Speed={Speed})";
	}

	public class OESAnimatedMesh : OESChunk {
		public bool Collidable;
		public IReadOnlyList<uint> IndexBuffer;
		public uint VertexCount;
		public float Speed;

		public OESAnimatedMesh(string typeCode, uint id) : base(typeCode, id) {}
		
		public OESAnimatedMesh(bool collidable, IReadOnlyList<uint> indexBuffer, uint vertexCount, float speed = 0) : base("amsh") {
			Collidable = collidable;
			IndexBuffer = indexBuffer;
			VertexCount = vertexCount;
			Speed = speed;
		}

		protected override void SerializeData(BinaryWriter bw) {
			bw.WriteBool(Collidable);
			bw.Write(IndexBuffer.Count);
			bw.Write(VertexCount);
			bw.Write(Speed);
			IndexBuffer.ForEach(bw.Write);
		}

		protected override void DeserializeData(BinaryReader br) {
			Collidable = br.ReadBool();
			var ibc = br.ReadInt32();
			VertexCount = br.ReadUInt32();
			Speed = br.ReadSingle();
			IndexBuffer = Enumerable.Range(0, ibc).Select(_ => br.ReadUInt32()).ToList();
		}

		public override string ToString() => $"OESAnimatedMesh(Collidable={Collidable}, IndexBuffer.Count={IndexBuffer.Count}, VertexCount={VertexCount}, Speed={Speed})";
	}

	public class OESAnimationBuffer : OESChunk {
		public IReadOnlyList<IReadOnlyList<float>> VertexBuffers;
		
		public OESAnimationBuffer(string typeCode, uint id) : base(typeCode, id) {}

		public OESAnimationBuffer(IReadOnlyList<IReadOnlyList<float>> vertexBuffers) : base("abuf") =>
			VertexBuffers = vertexBuffers;

		protected override void SerializeData(BinaryWriter bw) {
			bw.Write(VertexBuffers.Count);
			bw.Write(VertexBuffers[0].Count / 8);
			VertexBuffers.ForEach(frame => frame.ForEach(bw.Write));
		}

		protected override void DeserializeData(BinaryReader br) {
			var frameCount = br.ReadInt32();
			var elemCount = br.ReadInt32() * 8;
			VertexBuffers = Enumerable.Range(0, frameCount).Select(_ => Enumerable.Range(0, elemCount).Select(__ => br.ReadSingle()).ToList()).ToList();
		}

		public override string ToString() => $"OESAnimationBuffer(VertexBuffers.Count={VertexBuffers.Count}, VertexBufferCounts={VertexBuffers.Select(x => x.Count).ToArray().Stringify()})";
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