using System;
using System.Collections.Generic;
using OpenEQ.NsimGui;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
	public class GuiRenderer : IGuiRenderer {
		public int CreateTexture(TextureFormat format, int width, int height, byte[] pixels) {
			var tex = GL.GenTexture();
			GL.BindTexture(TextureTarget.Texture2D, tex);
			GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int) TextureMinFilter.Linear);
			GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int) TextureMagFilter.Linear);

			PixelInternalFormat pif;
			PixelFormat pf;
			switch(format) {
				case TextureFormat.Alpha:
					pif = PixelInternalFormat.Alpha;
					pf = PixelFormat.Alpha;
					break;
				case TextureFormat.Rgb:
					pif = PixelInternalFormat.Rgb;
					pf = PixelFormat.Rgb;
					break;
				case TextureFormat.Rgba:
					pif = PixelInternalFormat.Rgba;
					pf = PixelFormat.Rgba;
					break;
				default:
					throw new NotImplementedException();
			}
			
			GL.TexImage2D(TextureTarget.Texture2D, 0, pif, width, height, 0, pf, PixelType.UnsignedByte, pixels);
			GL.BindTexture(TextureTarget.Texture2D, 0);
			
			return tex;
		}

		public void DeleteTexture(int id) => GL.DeleteTexture(id);

		public void Draw(IReadOnlyList<DrawCommandSet> commandSets) {
			throw new System.NotImplementedException();
		}
	}
}