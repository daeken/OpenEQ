using System;
using System.Collections.Generic;
using System.Linq;
using MoreLinq;
using OpenTK.Graphics.OpenGL4;
using OpenEQ.Common;
using static OpenEQ.Engine.Globals;
using static System.Console;

namespace OpenEQ.Engine {
	public partial class EngineCore {
		const int maxLights = 64;
		
		FrameBuffer FBO;
		int QuadVAO;
		Program Program;


		void SetupDeferredPathway() {
			Resize += (_, __) => {
				if(FBO == null)
					FBO = new FrameBuffer(Width, Height,
						FrameBufferAttachment.Rgba, FrameBufferAttachment.Xyz, 
						FrameBufferAttachment.Depth);
				else
					FBO.Resize(Width, Height);
			};
			
			GL.BindVertexArray(QuadVAO = GL.GenVertexArray());
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

			Program = new Program(@"
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

struct Light {
	vec3 pos, color;
	float radius;
};

uniform sampler2D uColor, uPosition, uDepth;
uniform vec3 uAmbientColor;
uniform Light uLights[" + maxLights + @"];
uniform int uLightCount;
out vec3 color;

void main() {
	gl_FragDepth = texture(uDepth, vTexCoord).x; // Copy depth from FBO to screen depth buffer
	vec3 csv = texture(uColor, vTexCoord).rgb;
	vec3 pos = texture(uPosition, vTexCoord).xyz;
	vec3 accum = uAmbientColor;
	for(int i = 0; i < uLightCount; ++i) {
		Light light = uLights[i];
		float dist = length(light.pos - pos);
		accum += light.color * pow(1 - min(dist / light.radius, 1), 3);
	}
	color = csv * accum;
}
			");
		}

		void RenderDeferredPathway() {
			var screenDim = vec2(Width, Height) / 2;
			var projView = FpsCamera.Matrix * ProjectionMat;
			Vec2 screenPos(Vec3 wpos) {
				var ipos = projView * vec4(wpos, 1);
				return (ipos.XY / ipos.W + 1) * screenDim;
			}
			
			GL.Viewport(0, 0, Width, Height);
			FBO.Bind();
			GL.ClearColor(0, 0, 0, 1);
			GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);
		
			GL.Enable(EnableCap.CullFace);
			GL.Enable(EnableCap.DepthTest);
			GL.Disable(EnableCap.Blend);

			Mesh.SetProjectionView(projView);
		
			Models.ForEach(model => model.Draw(translucent: false));
		
			FrameBuffer.Unbind();
			
			GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);

			const int tileSize = 128;
			var tw = (int) Math.Ceiling((float) Width / tileSize);
			var th = (int) Math.Ceiling((float) Height / tileSize);
			var tiles = Enumerable.Range(0, tw * th).Select(i => new List<(double Dist, PointLight Light)>()).ToArray();

			foreach(var light in Lights) {
				var toLight = Camera.Position - light.Position;
				var tll = toLight.Length;
				if(tll > light.Radius) {
					var lspos = screenPos(light.Position);
					toLight /= tll;
					var cp = toLight.Y != 0 || toLight.Z != 0 ? vec3(1, 0, 0) : vec3(0, 1, 0);
					var perp = toLight.Cross(cp).Normalized;
					var espos = screenPos(light.Position + perp * light.Radius);
					var pradius = (espos - lspos).Length;
					if(lspos.X + pradius < 0 || lspos.Y + pradius < 0 || lspos.X - pradius > Width || lspos.Y - pradius > Height)
						continue;
					pradius *= pradius;
					for(var x = 0; x < tw; ++x)
						for(var y = 0; y < th; ++y) {
							var tilePos = (x * tileSize, y * tileSize);
							var delta = (lspos.X - max(tilePos.Item1, min(lspos.X, tilePos.Item1 + tileSize)), lspos.Y - max(tilePos.Item2, min(lspos.Y, tilePos.Item2 + tileSize)));
							if(delta.Item1 * delta.Item1 + delta.Item2 * delta.Item2 < pradius)
								tiles[x * th + y].Add((tll, light));
						}
				} else
					tiles.ForEach(tile => tile.Add((tll, light)));
			}

			tiles = tiles.Select(tile => tile.Count <= maxLights ? tile : tile.OrderBy(x => x.Dist).Take(maxLights).ToList()).ToArray();
			
			GL.Enable(EnableCap.DepthTest);
			GL.BindVertexArray(QuadVAO);
		
			Program.Use();
			Program.SetUniform("uAmbientColor", vec3(0.35));
			Program.SetTextures(0, FBO.Textures, "uColor", "uPosition", "uDepth");

			GL.Enable(EnableCap.ScissorTest);
			var ti = 0;
			for(var x = 0; x < tw; ++x) {
				for(var y = 0; y < th; ++y, ++ti) {
					var tile = tiles[ti];
					GL.Scissor(x * tileSize, y * tileSize, tileSize, tileSize);
					Program.SetUniform("uLightCount", tile.Count);
					tile.ForEach((tl, tli) => {
						var light = tl.Light;
						var prefix = $"uLights[{tli}].";
						Program.SetUniform(prefix + "pos", light.Position);
						Program.SetUniform(prefix + "color", light.Color);
						Program.SetUniform(prefix + "radius", light.Radius);
					});
					GL.DrawElements(PrimitiveType.Triangles, 6, DrawElementsType.UnsignedInt, IntPtr.Zero);
				}
			}
			GL.Disable(EnableCap.ScissorTest);
		
			GL.DepthMask(true);
		}
	}
}