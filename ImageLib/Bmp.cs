using System.IO;
using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Advanced;
using SixLabors.ImageSharp.Formats.Bmp;
using SixLabors.ImageSharp.PixelFormats;

namespace ImageLib {
	public static class Bmp {
		public static Image Load(string name, byte[] data) {
			using(var ms = new MemoryStream(data))
				using(var simage = new BmpDecoder().Decode<Rgb24>(Configuration.Default, ms)) {
					var pixels = new byte[simage.Width * simage.Height * 3];
					var span = simage.Frames[0].GetPixelSpan();
					var j = 0;
					for(var i = 0; i < simage.Width * simage.Height * 3; ++i) {
						var pixel = span[i];
						pixels[j++] = pixel.R;
						pixels[j++] = pixel.G;
						pixels[j++] = pixel.B;
					}
					return new Image(ColorMode.Rgb, (simage.Width, simage.Height), pixels, name);
				}
		}
	}
}