using System.IO;
using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Formats.Bmp;
using SixLabors.ImageSharp.PixelFormats;

namespace ImageLib {
	public static class Bmp {
		public static Image Load(byte[] data) {
			using(var ms = new MemoryStream(data))
				using(var simage = new BmpDecoder().Decode<Rgb24>(Configuration.Default, ms))
					return new Image(ColorMode.Rgb, (simage.Width, simage.Height), simage.Frames[0].SavePixelData());
		}
	}
}