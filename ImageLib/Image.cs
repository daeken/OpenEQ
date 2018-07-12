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