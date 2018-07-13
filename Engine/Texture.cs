using ImageLib;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
	public class Texture {
		readonly int Id;
		readonly bool Transparent;
		
		public Texture(Image image, bool transparent) {
			Transparent = transparent;
			GL.BindTexture(TextureTarget.Texture2D, Id = GL.GenTexture());
			var filter = (int) (transparent ? TextureMinFilter.Linear : TextureMinFilter.LinearMipmapLinear);
			GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, filter);
			GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, filter);
			if(!transparent) {
				GL.GetFloat((GetPName) 0x84FF, out float maxAniso);
				GL.TexParameter(TextureTarget.Texture2D, (TextureParameterName)All.TextureMaxAnisotropyExt, maxAniso);
			}

			var pif = PixelInternalFormat.Alpha;
			var pf = PixelFormat.Alpha;
			switch(image.ColorMode) {
				case ColorMode.Rgba:
					pif = PixelInternalFormat.Rgba;
					pf = PixelFormat.Rgba;
					break;
				case ColorMode.Rgb:
					pif = PixelInternalFormat.Rgb;
					pf = PixelFormat.Rgb;
					break;
				case ColorMode.Greyscale:
					pif = PixelInternalFormat.R8;
					pf = PixelFormat.Red;
					break;
			}
			GL.TexImage2D(TextureTarget.Texture2D, 0, pif, image.Size.Width, image.Size.Height, 0, pf, PixelType.UnsignedByte, image.Data);
			if(!transparent)
				GL.GenerateMipmap(GenerateMipmapTarget.Texture2D);
		}

		~Texture() => GL.DeleteTexture(Id);

		public void Use() => GL.BindTexture(TextureTarget.Texture2D, Id);
	}
}