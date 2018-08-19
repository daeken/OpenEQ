using System;
using System.Diagnostics;
using System.IO;
using System.Linq;

namespace ImageLib {
	public enum ColorMode {
		Greyscale,
		Rgb, 
		Rgba
	}
	
	public class Image {
		public readonly ColorMode ColorMode;
		public readonly (int Width, int Height) Size;
		public readonly byte[] Data;
		public readonly int Stride, PixelBytes;
		public readonly string Name;
		
		public Image(ColorMode colorMode, (int Width, int Height) size, byte[] data, string name = null) {
			ColorMode = colorMode;
			Size = size;
			Data = data;
			PixelBytes = PixelSize(ColorMode);
			Stride = Size.Width * PixelBytes;
			Debug.Assert(Data.Length == PixelBytes * Size.Width * Size.Height);
			Name = name;
		}

		public Image(ColorMode colorMode, (int Width, int Height) size, uint[] data, string name = null) {
			ColorMode = colorMode;
			Size = size;
			Debug.Assert(data.Length == Size.Width * Size.Height);
			Data = new byte[size.Width * size.Height * PixelSize(colorMode)];
			Buffer.BlockCopy(data, 0, Data, 0, 4 * size.Width * size.Height);
			Name = name;
		}

		public Image FlipY() {
			var stride = Size.Width * PixelSize(ColorMode);
			var temp = new byte[stride];
			for(var y = 0; y < Size.Height; ++y) {
				var inv = Size.Height - y - 1;
				if(inv <= y) break;
				Array.Copy(Data, stride * y, temp, 0, stride);
				Array.Copy(Data, stride * inv, Data, stride * y, stride);
				Array.Copy(temp, 0, Data, stride * inv, stride);
			}
			return this;
		}

		public void SwapRB() {
			if(ColorMode == ColorMode.Greyscale) return;

			var size = PixelSize(ColorMode);
			for(var i = 0; i < Data.Length; i += size) {
				var t = Data[i];
				Data[i] = Data[i + 2];
				Data[i + 2] = t;
			}
		}

		public Image Upscale(int factor, Action<Image, float, float, byte[], int> sampler) {
			if(factor <= 0) throw new NotImplementedException();
			if(factor == 1) return this;

			var ns = (Width: Size.Width * factor, Height: Size.Height * factor);
			var ps = PixelBytes;
			var nd = new byte[ns.Width * ns.Height * ps];
			
			var i = 0;
			for(var y = 0; y < ns.Height; ++y) {
				var v = y / (ns.Height - 1f);
				for(var x = 0; x < ns.Width; ++x) {
					var u = x / (ns.Width - 1f);
					sampler(this, u, v, nd, i);
					i += ps;
				}
			}
			
			return new Image(ColorMode, ns, nd, Name);
		}

		public static void SampleNearest(Image im, float u, float v, byte[] odata, int offset) {
			u *= im.Size.Width - 1;
			v *= im.Size.Height - 1;

			var x = (int) u;
			var y = (int) v;

			if(x + 1 < im.Size.Width && u - x > .5)
				x++;
			if(x + 1 < im.Size.Height && v - y > .5)
				y++;

			var pos = y * im.Stride + x * im.PixelBytes;
			for(var i = 0; i < im.PixelBytes; ++i)
				odata[offset + i] = im.Data[pos + i];
		}

		static float Mix(float a, float b, float x) => a * (1f - x) + b * x;

		static byte ClampByte(float v) {
			v = MathF.Round(v);
			if(v <= 0) return 0;
			if(v >= 255) return 255;
			return (byte) v;
		}

		public static void SampleBilinear(Image im, float u, float v, byte[] odata, int offset) {
			u *= im.Size.Width - 1;
			v *= im.Size.Height - 1;

			var x = (int) u;
			var y = (int) v;

			u -= x;
			v -= y;

			var bx = x + 1 == im.Size.Width;
			var by = y + 1 == im.Size.Height;
			var pos = y * im.Stride + x * im.PixelBytes;
			for(var i = 0; i < im.PixelBytes; ++i) {
				var a = im.Data[pos + i];
				var b = bx ? a : im.Data[pos + i + im.PixelBytes];
				var c = by ? a : im.Data[pos + i + im.Stride];
				var d = bx && by ? a : (bx ? c : (by ? b : im.Data[pos + i + im.PixelBytes + im.Stride]));
				odata[offset + i] = ClampByte(Mix(Mix(a, b, u), Mix(c, d, u), v));
			}
		}

		public Image UpscaleFfmpeg(int factor) {
			if(factor <= 0) throw new NotImplementedException();
			if(factor == 1) return this;

			var ns = (Width: Size.Width * factor, Height: Size.Height * factor);

			var tfn = Path.GetTempFileName() + ".png";
			using(var fp = File.OpenWrite(tfn))
				Png.Encode(this, fp);

			var otfn = Path.GetTempFileName() + ".png";
			Process.Start("ffmpeg", $"-i {tfn} -vf scale={ns.Width}:{ns.Height}:flags=lanczos {otfn}").WaitForExit();

			using(var fp = File.OpenRead(otfn))
				return Png.Decode(Name, fp);
		}


		public Image AlphaFromIntensity() {
			switch(ColorMode) {
				case ColorMode.Rgb:
					var odata = new byte[Size.Width * Size.Height * 4];
					var oi = 0;
					for(var i = 0; i < Size.Width * Size.Height * 3; ) {
						var r = Data[i++];
						var g = Data[i++];
						var b = Data[i++];
						odata[oi++] = r;
						odata[oi++] = g;
						odata[oi++] = b;
						odata[oi++] = ClampByte((r + g + b) / 3f);
					}
					return new Image(ColorMode.Rgba, Size, odata, Name);
				case ColorMode.Rgba:
					var ordata = Data.Select(x => x).ToArray();
					for(var i = 0; i < Size.Width * Size.Height * 4; i += 4) {
						var r = ordata[i + 0];
						var g = ordata[i + 1];
						var b = ordata[i + 2];
						ordata[i + 3] = ClampByte((r + g + b) / 3f / 255f * (ordata[i + 3] / 255f) * 255);
					}
					return new Image(ColorMode.Rgba, Size, ordata, Name);
				default: throw new NotImplementedException();
			}
		}
		public static int PixelSize(ColorMode mode) {
			switch(mode) {
				case ColorMode.Greyscale: return 1;
				case ColorMode.Rgb: return 3;
				case ColorMode.Rgba: return 4;
				default: throw new NotImplementedException();
			}
		}
	}
}