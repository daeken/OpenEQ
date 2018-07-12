using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using Be.IO;
using Force.Crc32;
using Ionic.Zlib;

namespace ImageLib {
	public static class Png {
		public static void Encode(Image image, Stream stream) {
			var bw = new BeBinaryWriter(stream, Encoding.Default, leaveOpen: true);
			bw.Write(new byte[] { 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A });

			void WriteChunk(string type, byte[] data) {
				Debug.Assert(type.Length == 4);
				bw.Write(data.Length);
				var td = Encoding.ASCII.GetBytes(type).Concat(data).ToArray();
				bw.Write(td);
				bw.Write(Crc32Algorithm.Compute(td));
			}

			void Chunk(string type, Action<BeBinaryWriter> func) {
				using(var ms = new MemoryStream())
					using(var sbw = new BeBinaryWriter(ms)) {
						func(sbw);
						sbw.Flush();
						WriteChunk(type, ms.ToArray());
					}
			}
			
			Chunk("IHDR", w => {
				w.Write(image.Size.Width);
				w.Write(image.Size.Height);
				w.Write((byte) 8);
				switch(image.ColorMode) {
					case ColorMode.Greyscale: w.Write((byte) 0); break;
					case ColorMode.Rgb: w.Write((byte) 2); break;
					case ColorMode.Rgba: w.Write((byte) 6); break;
					default: throw new NotImplementedException();
				}
				w.Write((byte) 0); // Compression mode
				w.Write((byte) 0); // Filter
				w.Write((byte) 0); // Interlace
			});

			var ps = Image.PixelSize(image.ColorMode);
			var stride = image.Size.Width * ps;
			var imem = new byte[image.Size.Height + image.Size.Width * image.Size.Height * ps]; // One byte per scanline for filter (0)
			for(var y = 0; y < image.Size.Height; ++y)
				Array.Copy(image.Data, y * stride, imem, y * stride + y + 1, stride);
			using(var ms = new MemoryStream()) {
				using(var ds = new ZlibStream(ms, CompressionMode.Compress, CompressionLevel.BestCompression, leaveOpen: true)) {
					ds.Write(imem, 0, imem.Length);
					ds.Flush();
				}
				ms.Flush();
				WriteChunk("IDAT", ms.ToArray());
			}

			Chunk("IEND", w => { });
			bw.Flush();
		}
	}
}