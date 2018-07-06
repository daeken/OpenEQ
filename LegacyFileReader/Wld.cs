using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using static System.Console;

namespace LegacyFileReader {
	public struct Fragment03 {
		public string[] Filenames;

		public override string ToString() => $"Fragment03(Filenames=[{string.Join(", ", Filenames.Select(x => $"'{x}'"))}]";
	}

	public struct Fragment04 {
		public uint FrameTime;
		public int[] References;

		public override string ToString() => $"Fragment04(FrameTime={FrameTime}, References=[{string.Join(", ", References.Select(x => x.ToString()))}])";
	}

	public struct Fragment05 {
		public int Reference;

		public override string ToString() => $"Fragment05(Reference={Reference})";
	}

	public struct Fragment15 {
		public int Reference;
		public float[] Position, Rotation, Scale;

		public override string ToString() => $"Fragment15(Reference={Reference}, Position={Position.Stringify()}, Rotation={Rotation.Stringify()}, Scale={Scale.Stringify()})";
	}

	public struct Fragment30 {
		public int Reference;
		public uint Flags;

		public override string ToString() => $"Fragment30(Reference={Reference}, Flags=0x{Flags:X8})";
	}

	public struct Fragment31 {
		public int[] References;

		public override string ToString() => $"Fragment31(References=[{string.Join(", ", References.Select(x => x.ToString()))}])";
	}
	
	public class Wld {
		static readonly byte[] StringHashKey = { 0x95, 0x3A, 0xC5, 0x2A, 0x95, 0x7A, 0x95, 0x6A };
		
		readonly Stream Fp;
		readonly BinaryReader Br;
		readonly bool NewFormat;

		readonly string StringHash;

		readonly (string Name, object Fragment)[] Fragments;
		
		public Wld(Stream fp) {
			Fp = fp;
			Br = new BinaryReader(fp);
			
			Debug.Assert(Br.ReadUInt32() == 0x54503D02);
			NewFormat = Br.ReadUInt32() != 0x00015500;

			var fragCount = Br.ReadUInt32();
			Fp.Position += 8;
			var stringHashSize = Br.ReadUInt32();
			Fp.Position += 4;

			StringHash = ReadDecodeString((int) stringHashSize);
			
			while(Fp.Position % 4 != 0)
				Fp.Position++;
			
			Fragments = new (string, object)[fragCount];

			for(var i = 0; i < fragCount; ++i) {
				var size = Br.ReadUInt32();
				var type = Br.ReadUInt32();
				var spos = Fp.Position;
				var nr = Br.ReadInt32();
				var name = nr < 0 && type != 0x35 ? GetString(-nr) : null;

				void Add<T>(T obj) {
					WriteLine(obj);
					Fragments[i] = (name, obj);
				}
				
				WriteLine($"Parsing fragment {i}, type 0x{type:X2} with size 0x{size:X} and name '{name}'");

				switch(type) {
					case 0x03: Add(Read03()); break;
					case 0x04: Add(Read04()); break;
					case 0x05: Add(Read05()); break;
					case 0x15: Add(Read15()); break;
					case 0x30: Add(Read30()); break;
					case 0x31: Add(Read31()); break;
					case 0x35:
						break;
					default:
						WriteLine($"Unhandled");
						break;
				}
				Debug.Assert(Fp.Position <= spos + size); // Make sure we didn't read past the end of a fragment
				Fp.Position = spos + size;
			}

			foreach(var (name, obj) in Fragments) {
				switch(obj) {
					case null: break;
					case Fragment04 f04:
						WriteLine($"0x04 fragment {f04}");
						WriteLine($"- References [{string.Join(", ", f04.References.Select(GetReference))}]");
						break;
					case Fragment15 f15:
						WriteLine($"0x15 fragment {f15}");
						WriteLine($"- Reference {GetReference(f15.Reference)}");
						break;
					case Fragment30 f30:
						WriteLine($"0x30 fragment {f30}");
						WriteLine($"- Reference {GetReference(f30.Reference)}");
						break;
				}
			}
		}

		Fragment03 Read03() =>
			new Fragment03 {
				Filenames = Enumerable.Range(0, Br.ReadInt32() + 1).Select(_ => ReadDecodeString(Br.ReadUInt16()).TrimEnd('\0')).ToArray()
			};
		Fragment04 Read04() {
			var flags = Br.ReadUInt32();
			var refCount = Br.ReadInt32();
			if((flags & (1 << 2)) != 0)
				Br.ReadUInt32();
			var frameTime = (flags & (1 << 3)) != 0 ? Br.ReadUInt32() : 0;
			var refs = Enumerable.Range(0, refCount).Select(_ => Br.ReadInt32()).ToArray();
			return new Fragment04 {
				FrameTime = frameTime, 
				References = refs
			};
		}
		Fragment05 Read05() {
			var reference = Br.ReadInt32();
			Br.ReadUInt32();
			return new Fragment05 { Reference = reference };
		}
		Fragment15 Read15() {
			var reference = Br.ReadInt32();
			Br.ReadUInt32();
			Br.ReadUInt32();
			var pos = new[] { Br.ReadSingle(), Br.ReadSingle(), Br.ReadSingle() };
			var rot = new[] { Br.ReadSingle(), Br.ReadSingle(), Br.ReadSingle() };
			var scale = new[] { Br.ReadSingle(), Br.ReadSingle(), Br.ReadSingle() };

			scale = scale[2] > 0.0001 ? new[] { scale[2], scale[2], scale[2] } : new[] { 1f, 1f, 1f };
			rot = new[] { rot[2] / 512 * 360 * MathF.PI / 180, rot[1] / 512 * 360 * MathF.PI / 180, rot[0] / 512 * 360 * MathF.PI / 180 };
			
			return new Fragment15 {
				Reference = reference, 
				Position = pos, 
				Rotation = rot, 
				Scale = scale
			};
		}
		Fragment30 Read30() {
			var existenceFlags = Br.ReadUInt32();
			var texFlags = Br.ReadUInt32();
			Br.ReadUInt32();
			Br.ReadSingle();
			Br.ReadSingle();
			if((existenceFlags & (1 << 1)) != 0) {
				Br.ReadUInt32();
				Br.ReadSingle();
			}
			var reference = Br.ReadInt32();
			return new Fragment30 {
				Reference = reference, 
				Flags = texFlags
			};
		}
		Fragment31 Read31() {
			Debug.Assert(Br.ReadUInt32() == 0);
			return new Fragment31 {
				References = Enumerable.Range(0, Br.ReadInt32()).Select(_ => Br.ReadInt32()).ToArray()
			};
		}

		object GetReference(int reference) {
			if(reference > 0)
				return Fragments[reference - 1];
			else {
				var name = GetString(-reference);
				WriteLine(name);
				return null;
			}
		}

		string ReadDecodeString(int size) => Encoding.ASCII.GetString(Br.ReadBytes(size).Select((x, i) => (byte) (x ^ StringHashKey[i % 8])).ToArray());

		string GetString(int pos) => StringHash.Substring(pos).Split('\0', 2)[0];
		string GetString(uint pos) => GetString((int) pos);
	}
}