using System;
using System.Diagnostics;
using System.IO;
using System.Text;
using static System.Console;

namespace ImageLib {
	[Flags]
	enum DdsFlags : uint {
		Caps = 1, 
		Height = 2, 
		Width = 4, 
		Pitch = 8, 
		PixelFormat = 0x1000, 
		MipmapCount = 0x20000, 
		LinearSize = 0x80000, 
		Depth = 0x800000
	}

	[Flags]
	enum DdsPfFlags : uint {
		AlphaPixels = 1, 
		Alpha = 2, 
		Fourcc = 4, 
		Rgb = 0x40, 
		Yuv = 0x200, 
		Luminance = 0x20000
	}
	
	public static class Dds {
		public static Image Decode(Stream stream) {
			var br = new BinaryReader(stream, Encoding.Default, leaveOpen: true);

			var magic = br.ReadBytes(4);
			Debug.Assert(Encoding.ASCII.GetString(magic) == "DDS ");

			// DDS Header
			var size = br.ReadUInt32();
			Debug.Assert(size == 124);
			var flags = (DdsFlags) br.ReadUInt32();
			var height = br.ReadUInt32();
			var width = br.ReadUInt32();
			var pitchOrLinear = br.ReadUInt32();
			var depth = br.ReadUInt32();
			var mipMapCount = br.ReadUInt32();
			br.BaseStream.Position += 44;
			
			// Pixelformat struct
			size = br.ReadUInt32();
			Debug.Assert(size == 32);
			var pflags = (DdsPfFlags) br.ReadUInt32();
			var fourcc = Encoding.ASCII.GetString(br.ReadBytes(4));
			var rgbBitcount = br.ReadUInt32();
			var rBitmask = br.ReadUInt32();
			var gBitmask = br.ReadUInt32();
			var bBitmask = br.ReadUInt32();
			var aBitmask = br.ReadUInt32();
			
			// Back to header
			var caps = br.ReadUInt32();
			var caps2 = br.ReadUInt32();
			br.BaseStream.Position += 12;

			WriteLine($"Fourcc '{fourcc}'");

			return null;
		}
	}
}