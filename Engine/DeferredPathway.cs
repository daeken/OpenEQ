using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using MoreLinq;
using OpenEQ.Common;
using OpenTK.Graphics.OpenGL4;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public partial class EngineCore {
		const int maxLights = 64;
		
		FrameBuffer FBO;
		Vao QuadVAO;
		Program Program;
		int[] UniformLocs;
		int UniformLC;

		bool DeferredSetup;

		void SetupDeferredPathway() {
			if(DeferredSetup || !DeferredEnabled)
				return;
			DeferredSetup = true;
			FBO = new FrameBuffer(Width, Height,
				FrameBufferAttachment.Rgb, FrameBufferAttachment.Xyz, 
				FrameBufferAttachment.Depth);
			Resize += (_, __) => FBO.Resize(Width, Height);

			QuadVAO = new Vao();
			QuadVAO.Attach(new Buffer<uint>(new[] { 0U, 1U, 2U, 3U, 4U, 5U }, BufferTarget.ElementArrayBuffer));
			QuadVAO.Attach(new Buffer<float>(new[] {
				-1f, -1f, 
				1f, -1f, 
				1f,  1f, 
					
				-1f, -1f, 
				1f,  1f, 
				-1f,  1f
			}), (0, typeof(Vector2)));

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

uniform mat4 uInvProjectionViewMat;
uniform sampler2D uColor, uNormal, uDepth;
uniform vec3 uAmbientColor;
uniform Light uLights[" + maxLights + @"];
uniform int uLightCount;
out vec4 color;

void main() {
	gl_FragDepth = texture(uDepth, vTexCoord).x; // Copy depth from FBO to screen depth buffer
	vec3 csv = texture(uColor, vTexCoord).rgb;
	vec3 normal = normalize(texture(uNormal, vTexCoord).xyz);
	vec4 sspos = uInvProjectionViewMat * (vec4(vTexCoord.xy, gl_FragDepth, 1) * 2 - 1);
	vec3 pos = sspos.xyz / sspos.w;
	vec3 accum = uAmbientColor;
	for(int i = 0; i < uLightCount; ++i) {
		Light light = uLights[i];
		vec3 toLight = light.pos - pos;
		float dist = length(toLight);
		float intensity = min(max(dot(normal, toLight / dist), 0.0), 1);
		accum += light.color * pow(1 - min(dist / light.radius, 1), 3) * intensity;
	}
	color = vec4(csv * accum, 1);
}
			");

			Program.Use();
			UniformLocs = Enumerable.Range(0, maxLights).Select(i => new[] {
				Program.GetUniform($"uLights[{i}].pos"), 
				Program.GetUniform($"uLights[{i}].color"), 
				Program.GetUniform($"uLights[{i}].radius")
			}).SelectMany(x => x).ToArray();
			UniformLC = Program.GetUniform("uLightCount");
		}

		void RenderDeferredPathway() {
			var screenDim = vec2(Width, Height) / 2;
			Matrix4x4.Invert(ProjectionView, out var invProjView);
			Vector2 screenPos(Vector3 wpos) {
				var ipos = Vector4.Transform(vec4(wpos, 1), ProjectionView);
				return (ipos.XY() / ipos.W).Add(1) * screenDim;
			}
			
			NoProfile("- G-buffer render", () => {
				GL.Viewport(0, 0, Width, Height);
				FBO.Bind();
				GL.ClearColor(0, 0, 0, 1);
				GL.ClearStencil(0);
				GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit | ClearBufferMask.StencilBufferBit);
		
				GL.Enable(EnableCap.CullFace);
				GL.Enable(EnableCap.DepthTest);
				GL.Disable(EnableCap.Blend);

				Models.ForEach(model => model.Draw(ProjectionView, forward: false));
				AniModels.ForEach(model => model.Draw(ProjectionView, forward: false));
		
				FrameBuffer.Unbind();
				GL.Finish();
			});
			
			GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);

			const int tileSize = 256;
			var tw = (int) Math.Ceiling((float) Width / tileSize);
			var th = (int) Math.Ceiling((float) Height / tileSize);
			IReadOnlyList<IEnumerable<(double Dist, PointLight Light)>> tiles = null;

			NoProfile("- Tile determination", () => {
				var cforward = Vector3.Transform(FpsCamera.Forward, Camera.LookRotation).Normalized();
				var cp = cforward.Y != 0 || cforward.Z != 0 ? vec3(1, 0, 0) : vec3(0, 1, 0);
				var perp = cforward.Cross(cp).Normalized();
				var tileLists = Enumerable.Range(0, tw * th).Select(i => new List<(double Dist, PointLight Light)>()).ToArray();
				foreach(var light in Lights) {
					var toLight = Camera.Position - light.Position;
					var tll = toLight.Length();
					if(tll > light.Radius) {
						var lspos = screenPos(light.Position);
						var ppos = Camera.Position + cforward * tll;
						var pradius = (screenDim - screenPos(ppos + perp * light.Radius)).Length();
						if(lspos.X + pradius < 0 || lspos.Y + pradius < 0 || lspos.X - pradius > Width || lspos.Y - pradius > Height)
							continue;
						pradius *= pradius;
						for(var x = 0; x < tw; ++x)
							for(var y = 0; y < th; ++y) {
								var tilePos = (x * tileSize, y * tileSize);
								var delta = (lspos.X - max(tilePos.Item1, min(lspos.X, tilePos.Item1 + tileSize)), lspos.Y - max(tilePos.Item2, min(lspos.Y, tilePos.Item2 + tileSize)));
								if(delta.Item1 * delta.Item1 + delta.Item2 * delta.Item2 < pradius)
									tileLists[x * th + y].Add((tll, light));
							}
					} else
						tileLists.ForEach(tile => tile.Add((tll, light)));
				}

				tiles = tileLists.Select(tile => tile.Count <= maxLights ? tile : tile.OrderBy(x => x.Item1).Take(maxLights)).ToList();
			});

			NoProfile("- Tile render", () => {
				GL.ClearColor(0.2f, 0.2f, 0.2f, 1);
				GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);
				GL.Enable(EnableCap.DepthTest);
				QuadVAO.Bind(() => {
					Program.Use();
					Program.SetUniform("uInvProjectionViewMat", invProjView);
					Program.SetUniform("uAmbientColor", vec3(0.25f));	
					Program.SetTextures(0, FBO.Textures, "uColor", "uNormal", "uDepth");

					GL.Enable(EnableCap.ScissorTest);
					var ti = 0;
					for(var x = 0; x < tw; ++x) {
						for(var y = 0; y < th; ++y, ++ti) {
							GL.Scissor(x * tileSize, y * tileSize, tileSize, tileSize);
							var tc = 0;
							tiles[ti].ForEach((tl, tli) => {
								var light = tl.Light;
								GL.Uniform3(UniformLocs[tc++], 1, ref light.Position.X);
								GL.Uniform3(UniformLocs[tc++], 1, ref light.Color.X);
								GL.Uniform1(UniformLocs[tc++], light.Radius);
							});
							GL.Uniform1(UniformLC, tc / 3);
							GL.DrawElements(PrimitiveType.Triangles, 6, DrawElementsType.UnsignedInt, IntPtr.Zero);
						}
					}
					GL.Disable(EnableCap.ScissorTest);
					GL.Finish();
				});
			});
		}
	}
}