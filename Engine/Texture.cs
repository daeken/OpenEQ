using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
	public class Texture {
		readonly int Id;
		readonly bool Transparent;
		
		public Texture((int, int) size, byte[] data, bool transparent) {
			Transparent = transparent;
			GL.BindTexture(TextureTarget.Texture2D, Id = GL.GenTexture());
			var filter = (int) (transparent ? TextureMinFilter.Linear : TextureMinFilter.LinearMipmapLinear);
			GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, filter);
			GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, filter);
			if(!transparent) {
				GL.GetFloat((GetPName) 0x84FF, out float maxAniso);
				GL.TexParameter(TextureTarget.Texture2D, (TextureParameterName)All.TextureMaxAnisotropyExt, maxAniso);
			}
			GL.TexImage2D(TextureTarget.Texture2D, 0, PixelInternalFormat.Rgba, size.Item1, size.Item2, 0, PixelFormat.Rgba, PixelType.UnsignedByte, data);
			if(!transparent)
				GL.GenerateMipmap(GenerateMipmapTarget.Texture2D);
		}

		~Texture() => GL.DeleteTexture(Id);

		public void Use() => GL.BindTexture(TextureTarget.Texture2D, Id);
	}
}