using System;
using System.Diagnostics;

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
		
		public Image(ColorMode colorMode, (int Width, int Height) size, byte[] data) {
			ColorMode = colorMode;
			Size = size;
			Data = data;
			Debug.Assert(Data.Length == PixelSize(ColorMode) * Size.Width * Size.Height);
		}

		public Image(ColorMode colorMode, (int Width, int Height) size, uint[] data) {
			ColorMode = colorMode;
			Size = size;
			Debug.Assert(data.Length == Size.Width * Size.Height);
			Data = new byte[size.Width * size.Height * PixelSize(colorMode)];
			Buffer.BlockCopy(data, 0, Data, 0, 4 * size.Width * size.Height);
		}

		public void FlipY() {
			var stride = Size.Width * PixelSize(ColorMode);
			var temp = new byte[stride];
			for(var y = 0; y < Size.Height; ++y) {
				var inv = Size.Height - y - 1;
				if(inv <= y) break;
				Array.Copy(Data, stride * y, temp, 0, stride);
				Array.Copy(Data, stride * inv, Data, stride * y, stride);
				Array.Copy(temp, 0, Data, stride * inv, stride);
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