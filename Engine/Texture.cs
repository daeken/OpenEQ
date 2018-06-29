using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
	public class Texture {
		readonly int Id;
		readonly bool Transparent;
		
		public Texture((int, int) size, byte[] data, bool transparent) {
			Transparent = transparent;
			GL.BindTexture(TextureTarget.Texture2D, Id = GL.GenTexture());
			GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int) TextureMinFilter.Linear);
			GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int) TextureMagFilter.Linear);
			GL.TexImage2D(TextureTarget.Texture2D, 0, PixelInternalFormat.Rgba, size.Item1, size.Item2, 0, PixelFormat.Rgba, PixelType.UnsignedByte, data);
			if(!transparent)
				GL.GenerateMipmap(GenerateMipmapTarget.Texture2D);
		}

		~Texture() => GL.DeleteTexture(Id);

		public void Use() => GL.BindTexture(TextureTarget.Texture2D, Id);
	}
}