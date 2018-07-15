using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using NsimGui;
using NsimGui.Widgets;
using OpenTK;
using OpenTK.Graphics;
using OpenTK.Graphics.OpenGL4;
using OpenTK.Input;
using OpenEQ.Common;
using Vector2 = System.Numerics.Vector2;
using Vector3 = System.Numerics.Vector3;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public partial class EngineCore : GameWindow {
		public readonly Gui Gui;
		
		readonly List<Model> Models = new List<Model>();

		readonly List<double> FrameTimes = new List<double>();
		
		readonly List<PointLight> Lights = new List<PointLight>();
		
		public EngineCore() : base(
			1280, 720, new GraphicsMode(new ColorFormat(8, 8, 8, 8), 16, 0), "OpenEQ", 
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

			Resize += (_, e) => {
				Gui.Dimensions = new Vector2(Width, Height);
				Gui.Scale = new Vector2(2f);
				ProjectionMat = Matrix4x4.CreatePerspectiveFieldOfView(45 * (MathF.PI / 180), (float) Width / Height, 1, 5000);
			};
			
			SetupDeferredPathway();
		}

		public void AddLight(Vector3 pos, float radius, float attenuation, Vector3 color) =>
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
			var pitchscale = .5f;
			var yawscale = 1.25f;
			var updatedCamera = false;
			foreach(var key in KeyState.Keys)
				switch(key) {
					case Key.W:
						movement += vec3(0, (float) e.Time * movescale, 0);
						break;
					case Key.S:
						movement += vec3(0, (float) -e.Time * movescale, 0);
						break;
					case Key.A:
						movement += vec3((float) -e.Time * movescale, 0, 0);
						break;
					case Key.D:
						movement += vec3((float) e.Time * movescale, 0, 0);
						break;
					case Key.Space:
						movement += vec3(0, 0, (float) e.Time * movescale);
						break;
					case Key.ShiftLeft:
						movement += vec3(0, 0, (float) -e.Time * movescale);
						break;
					case Key.Up:
						Camera.Look((float) e.Time * pitchscale, 0);
						updatedCamera = true;
						break;
					case Key.Down:
						Camera.Look((float) -e.Time * pitchscale, 0);
						updatedCamera = true;
						break;
					case Key.Left:
						Camera.Look(0, (float) e.Time * yawscale);
						updatedCamera = true;
						break;
					case Key.Right:
						Camera.Look(0, (float) -e.Time * yawscale);
						updatedCamera = true;
						break;
					case Key.Escape:
					case Key.Q:
						Exit();
						break;
				}
			if(movement.Length() > 0) {
				Camera.Move(movement);
				updatedCamera = true;
			}

			if(updatedCamera)
				Camera.Update();
		}

		protected override void OnRenderFrame(FrameEventArgs e) {
			if(FrameTimes.Count == 200)
				FrameTimes.RemoveAt(0);
			FrameTimes.Add(e.Time);

			Profile("Deferred render", RenderDeferredPathway);

			Profile("Forward render", () => {
				GL.Enable(EnableCap.Blend);
				GL.BlendFunc(BlendingFactor.SrcAlpha, BlendingFactor.OneMinusSrcAlpha);
				GL.Enable(EnableCap.DepthTest);
				GL.ActiveTexture(TextureUnit.Texture0);
				GL.DepthMask(false);
				Models.ForEach(model => model.Draw(translucent: true));
				GL.DepthMask(true);
				GL.Flush();
			});

			Gui.Render((float) e.Time);

			SwapBuffers();
		}
	}
}