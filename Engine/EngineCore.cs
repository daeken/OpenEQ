using System;
using System.Collections.Generic;
using System.Linq;
using NsimGui;
using NsimGui.Widgets;
using OpenTK;
using OpenTK.Graphics;
using OpenTK.Graphics.OpenGL4;
using OpenTK.Input;
using Vector2 = System.Numerics.Vector2;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class EngineCore : GameWindow {
		public readonly Gui Gui;
		
		readonly List<Model> Models = new List<Model>();

		readonly List<double> FrameTimes = new List<double>();
		
		FrameBuffer FBO;
		readonly int DeferredQuadVAO, DeferredSphereVAO, SphereElementCount;
		readonly Program DeferredAmbientProgram, DeferredSphereProgram;
		readonly List<PointLight> Lights = new List<PointLight>();
		
		public EngineCore() : base(
			1280, 720, new GraphicsMode(new ColorFormat(32), 24, 8), "OpenEQ", 
			GameWindowFlags.Default, DisplayDevice.Default, 4, 1, GraphicsContextFlags.ForwardCompatible
		) {
			VSync = VSyncMode.Off;
			Stopwatch.Start();
			Gui = new Gui(new GuiRenderer()) {
				new Window("Status") {
					new Size(500, 100), 
					new Text(() => $"Position {Camera.Position}"), 
					new Text(() => $"FPS {(FrameTimes.Count == 0 ? 0 : 1 / (FrameTimes.Sum() / (double) FrameTimes.Count))}")
				}
			};
			
			MouseMove += (_, e) => Gui.MousePosition = (e.X, e.Y);
			MouseDown += (_, e) => UpdateMouseButton(e.Button, true);
			MouseUp += (_, e) => UpdateMouseButton(e.Button, false);
			MouseWheel += (_, e) => Gui.WheelDelta += e.Delta;
			
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

			var (vb, ib) = Helpers.MakeSphereGeometry(10, 10);
			SphereElementCount = ib.Length;
			GL.BindVertexArray(DeferredSphereVAO = GL.GenVertexArray());
			GL.BindBuffer(BufferTarget.ArrayBuffer, GL.GenBuffer());
			GL.BufferData(BufferTarget.ArrayBuffer, vb.Length * 4, vb, BufferUsageHint.StaticDraw);
			GL.BindBuffer(BufferTarget.ElementArrayBuffer, GL.GenBuffer());
			GL.BufferData(BufferTarget.ElementArrayBuffer, ib.Length * 4, ib, BufferUsageHint.StaticDraw);
			
			GL.EnableVertexAttribArray(0);
			GL.VertexAttribPointer(0, 3, VertexAttribPointerType.Float, false, 0, 0);
			
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

uniform sampler2D uColor, uPosition, uNormal, uDepth;
uniform vec3 uAmbientColor;
out vec3 color;

vec3 csv, N;

float calcDiffuse(vec3 lightDir) {
	vec3 toLight = normalize(lightDir);
	return clamp(dot(N, toLight), 0, 1);
}

void main() {
	gl_FragDepth = texture(uDepth, vTexCoord).r;
	csv = texture(uColor, vTexCoord).rgb;
	N = texture(uNormal, vTexCoord).xyz;

	float diffuse = calcDiffuse(vec3(10, 5, 25)) + calcDiffuse(vec3(-15, 5, 25)) + calcDiffuse(vec3(-5, -11, 7));
	
	color = csv * uAmbientColor * (.7 + diffuse / 5);
}
			");
			
			DeferredSphereProgram = new Program(@"
#version 410
precision highp float;
in vec4 aPosition;
uniform mat4 uProjectionViewMat, uModelMat;
void main() {
	gl_Position = uProjectionViewMat * uModelMat * aPosition;
}
			", @"
#version 410
precision highp float;
out vec3 color;
void main() {
	color = vec3(.1);
}
			");
		}

		public void AddLight(Vec3 pos, float radius, float attenuation, Vec3 color) =>
			Lights.Add(new PointLight(pos, radius, attenuation, color));

		public void Add(Model model) => Models.Add(model);

		void UpdateMouseButton(MouseButton button, bool state) {
			switch(button) {
				case MouseButton.Left:
					Gui.MouseLeft = state;
					break;
				case MouseButton.Right:
					Gui.MouseRight = state;
					break;
			}
		}
		
		readonly Dictionary<Key, bool> KeyState = new Dictionary<Key, bool>();

		protected override void OnKeyDown(KeyboardKeyEventArgs e) => KeyState[e.Key] = true;
		protected override void OnKeyUp(KeyboardKeyEventArgs e) => KeyState.Remove(e.Key);

		protected override void OnUpdateFrame(FrameEventArgs e) {
			if(KeyState.Count == 0)
				return;
			var movement = vec3();
			var movescale = KeyState.Keys.Contains(Key.WinLeft) ? 250 : 30;
			var pitchscale = .5;
			var yawscale = 1;
			var updatedCamera = false;
			foreach(var key in KeyState.Keys)
				switch(key) {
					case Key.W:
						movement += vec3(0, -e.Time * movescale, 0);
						break;
					case Key.S:
						movement += vec3(0, e.Time * movescale, 0);
						break;
					case Key.A:
						movement += vec3(e.Time * movescale, 0, 0);
						break;
					case Key.D:
						movement += vec3(-e.Time * movescale, 0, 0);
						break;
					case Key.Space:
						movement += vec3(0, 0, e.Time * movescale);
						break;
					case Key.ShiftLeft:
						movement += vec3(0, 0, -e.Time * movescale);
						break;
					case Key.Up:
						Camera.Look(-e.Time * pitchscale, 0);
						updatedCamera = true;
						break;
					case Key.Down:
						Camera.Look(e.Time * pitchscale, 0);
						updatedCamera = true;
						break;
					case Key.Left:
						Camera.Look(0, e.Time * yawscale);
						updatedCamera = true;
						break;
					case Key.Right:
						Camera.Look(0, -e.Time * yawscale);
						updatedCamera = true;
						break;
					case Key.Escape:
					case Key.Q:
						Exit();
						break;
				}
			if(movement.Length > 0) {
				Camera.Move(movement);
				updatedCamera = true;
			}

			if(updatedCamera)
				Camera.Update();
		}

		protected override void OnResize(EventArgs e) {
			Gui.Dimensions = new Vector2(Width, Height);
			Gui.Scale = new Vector2(2f);
			ProjectionMat = Mat4.Perspective(45 * (Math.PI / 180), (double) Width / Height, 1, 5000);
			
			if(FBO == null)
				FBO = new FrameBuffer(Width, Height,
					FrameBufferAttachment.Rgba, FrameBufferAttachment.Xyz, FrameBufferAttachment.Xyz, 
					FrameBufferAttachment.Depth);
			else
				FBO.Resize(Width, Height);
		}

		protected override void OnRenderFrame(FrameEventArgs e) {
			if(FrameTimes.Count == 200)
				FrameTimes.RemoveAt(0);
			FrameTimes.Add(e.Time);
			
			GL.StencilMask(0);
			GL.Viewport(0, 0, Width, Height);
			FBO.Bind();
			GL.ClearColor(0, 0, 0, 1);
			GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);
			
			GL.Enable(EnableCap.CullFace);
			GL.Enable(EnableCap.DepthTest);
			GL.DepthFunc(DepthFunction.Less);
			GL.Disable(EnableCap.Blend);

			var projView = FpsCamera.Matrix * ProjectionMat;
			Mesh.SetProjectionView(projView);
			
			Models.ForEach(model => model.Draw());
			
			FrameBuffer.Unbind();
			
			GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);
			
			//GL.DepthMask(false);
			GL.Enable(EnableCap.DepthTest);
			GL.BindVertexArray(DeferredQuadVAO);
			
			DeferredAmbientProgram.Use();
			DeferredAmbientProgram.SetUniform("uAmbientColor", vec3(0.35));
			DeferredAmbientProgram.SetTextures(0, FBO.Textures, "uColor", "uPosition", "uNormal", "uDepth");
			GL.DrawElements(PrimitiveType.Triangles, 6, DrawElementsType.UnsignedInt, IntPtr.Zero);

			GL.Enable(EnableCap.Blend);
			GL.BlendFunc(BlendingFactor.One, BlendingFactor.One);

			GL.DepthMask(false);
			GL.Disable(EnableCap.DepthTest);
			PointLight.SetupFinal(FBO.Textures, Camera.Position);
			/*DeferredSphereProgram.Use();
			DeferredSphereProgram.SetUniform("uProjectionViewMat", projView);
			GL.Enable(EnableCap.StencilTest);
			GL.StencilMask(0xFFFFFFFFU);*/
			GL.Enable(EnableCap.ScissorTest);
			var screenDim = vec2(Width, Height) / 2;
			foreach(var light in Lights) {
				/*GL.Clear(ClearBufferMask.StencilBufferBit);
				var lmMat = Mat4.Scale(vec3(light.Radius * 1.1)) * Mat4.Translation(light.Position);
				DeferredSphereProgram.Use();
				DeferredSphereProgram.SetUniform("uModelMat", lmMat);
				GL.BindVertexArray(DeferredSphereVAO);
				GL.Enable(EnableCap.DepthTest);
				GL.CullFace(CullFaceMode.Back);
				GL.ColorMask(false, false, false, false);
				GL.DepthFunc(DepthFunction.Lequal);
				GL.StencilFunc(StencilFunction.Always, 0, 0);
				GL.StencilOpSeparate(StencilFace.Front, StencilOp.Keep, StencilOp.Incr, StencilOp.Keep);
				GL.DrawElements(PrimitiveType.Triangles, SphereElementCount, DrawElementsType.UnsignedInt, IntPtr.Zero);
				
				GL.CullFace(CullFaceMode.Front);
				GL.DepthFunc(DepthFunction.Gequal);
				GL.StencilFunc(StencilFunction.Equal, 0, 0xFFFFFFFFU);
				GL.StencilOpSeparate(StencilFace.Back, StencilOp.Zero, StencilOp.Zero, StencilOp.Incr);
				GL.DrawElements(PrimitiveType.Triangles, SphereElementCount, DrawElementsType.UnsignedInt, IntPtr.Zero);*/
				
				/*GL.CullFace(CullFaceMode.Back);
				GL.ColorMask(true, true, true, true);
				GL.Disable(EnableCap.DepthTest);
				GL.StencilFunc(StencilFunction.Notequal, 0, 0xFFFFFFFFU);*/

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
			/*GL.CullFace(CullFaceMode.Back);
			GL.StencilMask(0);
			GL.Disable(EnableCap.StencilTest);
			GL.DepthFunc(DepthFunction.Lequal);*/
			
			GL.DepthMask(true);

			Gui.Render((float) e.Time);

			SwapBuffers();
		}
	}
}