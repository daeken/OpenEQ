using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using NsimGui;
using OpenEQ.Common;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
	public class GuiRenderer : IGuiRenderer {
		readonly Program Program;
		
		public GuiRenderer() {
			Program = new Program(@"
#version 410
precision highp float;
layout (location = 0) in vec2 aPosition;
layout (location = 1) in vec2 aUV;
layout (location = 2) in vec4 aColor;
uniform mat4 uProjectionMat;
out vec2 vUV;
out vec4 vColor;
void main() {
	gl_Position = uProjectionMat * vec4(aPosition, 0.0, 1.0);
	vUV = aUV;
	vColor = aColor;
}
			", @"
#version 410
precision highp float;
in vec2 vUV;
in vec4 vColor;
layout (location = 0) out vec4 color;
uniform sampler2D uTex;
void main() {
	color = vColor * texture(uTex, vUV);
}
			");
		}
		
		public int CreateTexture(TextureFormat format, int width, int height, byte[] pixels) {
			var tex = GL.GenTexture();
			GL.BindTexture(TextureTarget.Texture2D, tex);
			GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int) TextureMinFilter.Linear);
			GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int) TextureMagFilter.Linear);
			
			PixelInternalFormat pif;
			PixelFormat pf;
			switch(format) {
				case TextureFormat.Alpha:
					pif = PixelInternalFormat.Rgba;
					pf = PixelFormat.Rgba;
					pixels = pixels.Select(x => new byte[] { 255, 255, 255, x }).SelectMany(x => x).ToArray();
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

		public void Draw((float, float) dimensions, IReadOnlyList<DrawCommandSet> commandSets) {
			GL.Enable(EnableCap.Blend);
			GL.Disable(EnableCap.DepthTest);
			GL.Disable(EnableCap.CullFace);
			GL.Enable(EnableCap.ScissorTest);
			
			GL.BlendFunc(BlendingFactorSrc.SrcAlpha, BlendingFactorDest.OneMinusSrcAlpha);
			
			Program.Use();
			GL.ActiveTexture(TextureUnit.Texture0);
			Program.SetUniform("uTex", 0);
			Program.SetUniform("uProjectionMat", new Matrix4x4(
				2f / dimensions.Item1, 0, 0, 0, 
				0, 2f / -dimensions.Item2, 0, 0, 
				0, 0, -1, 0, 
				-1, 1, 0, 1
			));

			foreach(var cset in commandSets) {
				var vao = new Vao();
				var indexBuffer = new Buffer<ushort>(cset.IBufferData, BufferTarget.ElementArrayBuffer, BufferUsageHint.StreamDraw);
				var vertexBuffer = new Buffer<byte>(cset.VBufferData, usage: BufferUsageHint.StreamDraw);
				vao.Attach(indexBuffer);
				vao.Attach(vertexBuffer, (0, VertexAttribPointerType.Float, 2), (1, VertexAttribPointerType.Float, 2), (2, VertexAttribPointerType.UnsignedByte, 4));
				vao.Bind(() => {
					foreach(var cmd in cset.Commands) {
						GL.BindTexture(TextureTarget.Texture2D, cmd.TextureId);
						GL.Scissor(cmd.Scissor.X, cmd.Scissor.Y, cmd.Scissor.Width, cmd.Scissor.Height);
						GL.DrawElements(PrimitiveType.Triangles, cmd.ElementCount, DrawElementsType.UnsignedShort, cmd.IndexOffset * 2);
					}
				});
				vao.Destroy();
				indexBuffer.Destroy();
				vertexBuffer.Destroy();
			}
			
			GL.Disable(EnableCap.Blend);
			GL.Disable(EnableCap.ScissorTest);
		}
	}
}