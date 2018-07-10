using System;
using OpenTK.Graphics.OpenGL4;
using static OpenEQ.Engine.Globals;
using static System.Console;

namespace OpenEQ.Engine {
	public partial class EngineCore {
		FrameBuffer FBO;
		int DeferredQuadVAO;
		Program DeferredAmbientProgram;


		void SetupDeferredPathway() {
			Resize += (_, __) => {
				if(FBO == null)
					FBO = new FrameBuffer(Width, Height,
						FrameBufferAttachment.Rgba, FrameBufferAttachment.Xyz, 
						FrameBufferAttachment.Depth);
				else
					FBO.Resize(Width, Height);
			};
			
			GL.BindVertexArray(DeferredQuadVAO = GL.GenVertexArray());
			GL.BindBuffer(BufferTarget.ArrayBuffer, GL.GenBuffer());
			GL.BufferData(BufferTarget.ArrayBuffer, 6 * 2 * 4, new[] {
				-1f, -1f, 
				1f, -1f, 
				1f,  1f, 
					
				-1f, -1f, 
				1f,  1f, 
				-1f,  1f
			}, BufferUsageHint.StaticDraw);
			GL.BindBuffer(BufferTarget.ElementArrayBuffer, GL.GenBuffer());
			GL.BufferData(BufferTarget.ElementArrayBuffer, 6 * 4, new[] { 0, 1, 2, 3, 4, 5 }, BufferUsageHint.StaticDraw);
			
			GL.EnableVertexAttribArray(0);
			GL.VertexAttribPointer(0, 2, VertexAttribPointerType.Float, false, 0, 0);

			DeferredAmbientProgram = new Program(@"
#version 410
precision highp float;
in vec2 aPosition;
out vec2 vTexCoord;
void main() {
	gl_Position = vec4(aPosition, 0.0, 1.0);
	vTexCoord = aPosition.xy * 0.5 + 0.5;
}
			", @"
#version 410
precision highp float;
in vec2 vTexCoord;

uniform sampler2D uColor, uPosition, uDepth;
uniform vec3 uAmbientColor;
out vec3 color;

void main() {
	//gl_FragDepth = texture(uDepth, vTexCoord).r; // Copy depth from FBO to screen depth buffer
	color = texture(uColor, vTexCoord).rgb * uAmbientColor;
}
			");
		}

		void RenderDeferredPathway() {
			GL.Viewport(0, 0, Width, Height);
			FBO.Bind();
			GL.ClearColor(0, 0, 0, 1);
			GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);
			
			GL.Enable(EnableCap.CullFace);
			GL.Enable(EnableCap.DepthTest);
			GL.Disable(EnableCap.Blend);

			var projView = FpsCamera.Matrix * ProjectionMat;
			Mesh.SetProjectionView(projView);
			
			Models.ForEach(model => model.Draw());
			
			FrameBuffer.Unbind();
			
			GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);
			
			GL.Enable(EnableCap.DepthTest);
			GL.BindVertexArray(DeferredQuadVAO);
			
			DeferredAmbientProgram.Use();
			DeferredAmbientProgram.SetUniform("uAmbientColor", vec3(0.35));
			DeferredAmbientProgram.SetTextures(0, FBO.Textures, "uColor", "uPosition", "uDepth");
			GL.DrawElements(PrimitiveType.Triangles, 6, DrawElementsType.UnsignedInt, IntPtr.Zero);

			GL.Enable(EnableCap.Blend);
			GL.BlendFunc(BlendingFactor.One, BlendingFactor.One);

			GL.DepthMask(false);
			GL.Disable(EnableCap.DepthTest);
			PointLight.SetupFinal(FBO.Textures, Camera.Position);
			GL.Enable(EnableCap.ScissorTest);
			var screenDim = vec2(Width, Height) / 2;
			foreach(var light in Lights) {
				Vec2 screenPos(Vec3 wpos) {
					var ipos = projView * vec4(wpos, 1);
					return (ipos.XY / ipos.W + 1) * screenDim;
				}

				var toLight = Camera.Position - light.Position;
				var tll = toLight.Length;
				if(tll > light.Radius) {
					var lspos = screenPos(light.Position);
					toLight /= tll;
					var cp = toLight.Y != 0 || toLight.Z != 0 ? vec3(1, 0, 0) : vec3(0, 1, 0);
					var perp = toLight.Cross(cp).Normalized;
					var espos = screenPos(light.Position + perp * light.Radius);
					var pradius = (espos - lspos).Length;
					if(pradius < 100)
						continue;
					var bl = lspos - pradius;
					var tr = lspos + pradius;
					if(tr.X < 0 || tr.Y < 0 || bl.X > Width || bl.Y > Height)
						continue;
					
					tr -= bl;

					GL.Scissor((int) max(bl.X, 0), (int) max(bl.Y, 0), (int) min(tr.X, Width - bl.X) + 1,
						(int) min(tr.Y, Height - bl.Y) + 1);
				} else
					GL.Scissor(0, 0, Width, Height);

				GL.BindVertexArray(DeferredQuadVAO);
				light.SetupIndividual();
				GL.DrawElements(PrimitiveType.Triangles, 6, DrawElementsType.UnsignedInt, IntPtr.Zero);
			}
			GL.Disable(EnableCap.ScissorTest);
			
			GL.DepthMask(true);
		}
	}
}