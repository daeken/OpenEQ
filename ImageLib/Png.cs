using System;
using System.Collections.Generic;
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

		public static Image Decode(Stream stream) {
			var br = new BeBinaryReader(stream, Encoding.Default, leaveOpen: true);

			ColorMode colorMode = ColorMode.Greyscale;
			var size = (Width: 0, Height: 0);
			byte[] data = null;

			var header = br.ReadBytes(8);

			var idats = new List<byte[]>();

			var running = true;
			while(running) {
				var dlen = br.ReadInt32();
				var type = Encoding.ASCII.GetString(br.ReadBytes(4));
				switch(type) {
					case "IHDR":
						size = (br.ReadInt32(), br.ReadInt32());
						br.ReadByte();
						switch(br.ReadByte()) {
							case 0: colorMode = ColorMode.Greyscale; break;
							case 2: colorMode = ColorMode.Rgb; break;
							case 6: colorMode = ColorMode.Rgba; break;
							default: throw new NotImplementedException();
						}
						data = new byte[size.Width * size.Height * Image.PixelSize(colorMode)];
						br.ReadByte();
						br.ReadByte();
						br.ReadByte();
						break;
					case "IDAT":
						idats.Add(br.ReadBytes(dlen));
						break;
					case "IEND":
						running = false;
						break;
					default:
						br.ReadBytes(dlen);
						break;
				}
				br.ReadUInt32();
			}

			var idata = idats.SelectMany(x => x).ToArray();
			using(var ms = new MemoryStream())
				using(var zs = new ZlibStream(ms, CompressionMode.Decompress)) {
					zs.Write(idata, 0, idata.Length);
					zs.Flush();
					ms.Flush();
					var tdata = ms.GetBuffer();
					var ps = Image.PixelSize(colorMode);
					var stride = size.Width * ps;
					for(var y = 0; y < size.Height; ++y) {
						Array.Copy(tdata, y * stride + y + 1, data, stride * y, stride);
						switch(tdata[y * stride + y]) {
							case 0: break;
							case 1: {
								for(var x = ps; x < stride; ++x)
									data[y * stride + x] = unchecked((byte) (data[y * stride + x] + data[y * stride + x - ps]));
								break;
							}
							case 2: {
								if(y == 0) break;
								for(var x = 0; x < stride; ++x)
									data[y * stride + x] = unchecked((byte) (data[y * stride + x] + data[y * stride + x - stride]));
								break;
							}
							case 4: {
								byte Paeth(byte a, byte b, byte c) {
									int p = a + b - c, pa = p > a ? p - a : a - p, pb = p > b ? p - b : b - p, pc = p > c ? p - c : c - p;
									return pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
								}
								for(var x = 0; x < stride; ++x) {
									byte mod = 0;
									if(x < ps) {
										if(y > 0)
											mod = Paeth(0, data[(y - 1) * stride + x], 0);
									} else {
										mod = y == 0
											? Paeth(data[y * stride + x - ps], 0, 0)
											: Paeth(data[y * stride + x - ps], data[(y - 1) * stride + x], data[(y - 1) * stride + x - ps]);
									}

									data[y * stride + x] = unchecked((byte) (data[y * stride + x] + mod));
								}
								break;
							}
							case byte x:
								throw new NotImplementedException($"Unsupported filter mode {x}");
						}
					}
				}
			
			return new Image(colorMode, size, data);
		}
	}
}