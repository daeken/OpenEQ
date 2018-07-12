using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text;
using MoreLinq;

namespace OpenEQ.Common {
	public abstract class OESChunk : IEnumerable<OESChunk> {
		static uint IdGen;
		
		public readonly string TypeCode;
		public readonly uint Id;
		public readonly List<OESChunk> Children = new List<OESChunk>();

		public OESChunk(string typeCode) {
			TypeCode = typeCode;
			Id = ++IdGen;
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

		protected abstract void SerializeData(BinaryWriter bw);
	}

	public class OESMaterial : OESChunk {
		public bool AlphaMask, Transparent, Emissive;

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
	}

	public class OESEffect : OESChunk {
		public string Name;

		public OESEffect(string name) : base("fx") => Name = name;
		
		protected override void SerializeData(BinaryWriter bw) {
			bw.WriteString(Name);
			bw.Write(0U);
		}
	}

	public class OESTexture : OESChunk {
		public string Filename;

		public OESTexture(string filename) : base("tex") => Filename = filename;

		protected override void SerializeData(BinaryWriter bw) => bw.WriteString(Filename);
	}

	public class OESZone : OESChunk {
		public string Name;

		public OESZone(string name) : base("zone") => Name = name;

		protected override void SerializeData(BinaryWriter bw) => bw.WriteString(Name);
	}

	public class OESRegion : OESChunk {
		public OESRegion() : base("regn") {}
		
		protected override void SerializeData(BinaryWriter bw) => throw new System.NotImplementedException();
	}

	public class OESCharacter : OESChunk {
		public OESCharacter() : base("char") {}
		
		protected override void SerializeData(BinaryWriter bw) => throw new System.NotImplementedException();
	}

	public class OESObject : OESChunk {
		public string Name;

		public OESObject(string name = null) : base("obj") => Name = name;

		protected override void SerializeData(BinaryWriter bw) => bw.WriteString(Name ?? "");
	}

	public class OESInstance : OESChunk {
		public OESObject Object;
		public Vec3 Position, Scale;
		public Vec4 Rotation;
		public string SkinName;

		public OESInstance(OESObject obj, Vec3 position, Vec3 scale, Vec4 rotation, string skinName = null) : base("inst") {
			Object = obj;
			Position = position;
			Scale = scale;
			Rotation = rotation;
			SkinName = skinName;
		}

		protected override void SerializeData(BinaryWriter bw) {
			bw.Write(Object.Id);
			bw.WriteString(SkinName ?? "");
			bw.Write(Position);
			bw.Write(Scale);
			bw.Write(Rotation);
		}
	}

	public class OESLight : OESChunk {
		public Vec3 Position, Color;
		public float Radius, Attenuation;

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
	}

	public class OESSkin : OESChunk {
		public string Name;

		public OESSkin(string name = null) : base("skin") => Name = name;

		protected override void SerializeData(BinaryWriter bw) => bw.Write(Name ?? "");
	}

	public class OESStaticMesh : OESChunk {
		public bool Collidable;
		public IReadOnlyList<uint> IndexBuffer;
		public IReadOnlyList<float> VertexBuffer;

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
	}
	
	public static class OESFile {
		public static OESChunk Read(Stream fp) => throw new NotImplementedException();

		public static void Write(Stream fp, OESChunk chunk) {
			using(var bw = new BinaryWriter(fp))
				chunk.Serialize(bw);
		}
	}
}