using System;
using System.Linq;
using MoreLinq;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
	public enum FrameBufferAttachment {
		Rgba, 
		Rgb, 
		Depth, 
		Depth16, 
		DepthStencil
	}
	
	public class FrameBuffer {
		public FrameBufferAttachment[] Attachments;
		public int FBO;
		public int[] Textures;
		
		public static void Unbind() => GL.BindFramebuffer(FramebufferTarget.FramebufferExt, 0);

		public FrameBuffer(int width, int height, params FrameBufferAttachment[] attachments) {
			Attachments = attachments;
			FBO = GL.GenFramebuffer();
			Resize(width, height);
		}

		public void Resize(int width, int height) {
			Textures?.ForEach(GL.DeleteTexture);

			Bind();
			var colors = 0;
			Textures = Attachments.Select((att, i) => {
				var tex = GL.GenTexture();
				GL.BindTexture(TextureTarget.Texture2D, tex);
				var pif = PixelInternalFormat.Rgb;
				var pf = PixelFormat.Rgb;
				var glatt = FramebufferAttachment.Aux0;
				var pt = PixelType.HalfFloat;
				switch(att) {
					case FrameBufferAttachment.Depth:
						pif = PixelInternalFormat.DepthComponent;
						pf = PixelFormat.DepthComponent;
						glatt = FramebufferAttachment.DepthAttachment;
						pt = PixelType.Float;
						break;
					case FrameBufferAttachment.Depth16:
						pif = PixelInternalFormat.DepthComponent16;
						pf = PixelFormat.DepthComponent;
						glatt = FramebufferAttachment.DepthAttachment;
						break;
					case FrameBufferAttachment.DepthStencil:
						pif = PixelInternalFormat.Depth24Stencil8;
						pf = PixelFormat.DepthStencil;
						pt = PixelType.UnsignedInt248;
						glatt = FramebufferAttachment.DepthStencilAttachment;
						break;
					case FrameBufferAttachment.Rgba:
						pif = PixelInternalFormat.Rgba;
						pf = PixelFormat.Rgba;
						break;
					case FrameBufferAttachment.Rgb:
						break;
				}
				if(glatt == FramebufferAttachment.Aux0)
					glatt = FramebufferAttachment.ColorAttachment0 + colors++;
				GL.TexImage2D(TextureTarget.Texture2D, 0, pif, width, height, 0, pf, pt, IntPtr.Zero);
				GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int) TextureMinFilter.Linear);
				GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMagFilter, (int) TextureMagFilter.Linear);
				GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureWrapS, (int) TextureWrapMode.ClampToEdge);
				GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureWrapT, (int) TextureWrapMode.ClampToEdge);
				if(att == FrameBufferAttachment.Depth || att == FrameBufferAttachment.Depth16)
					GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureCompareMode, (int) TextureCompareMode.CompareRefToTexture);
				GL.FramebufferTexture2D(FramebufferTarget.FramebufferExt, glatt, TextureTarget.Texture2D, tex, 0);
				return tex;
			}).ToArray();
			Unbind();
		}
		
		public void Bind() {
			GL.BindFramebuffer(FramebufferTarget.FramebufferExt, FBO);
			var bufs = Attachments.Count(x => x != FrameBufferAttachment.Depth && x != FrameBufferAttachment.Depth16 && x != FrameBufferAttachment.DepthStencil);
			GL.DrawBuffers(bufs, Enumerable.Range(0, bufs).Select(x => DrawBuffersEnum.ColorAttachment0 + x).ToArray());
		}
	}
}