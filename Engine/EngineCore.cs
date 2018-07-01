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
		
		public EngineCore() : base(
			1280, 720, new GraphicsMode(new ColorFormat(8, 8, 8, 8), 32), "OpenEQ", 
			GameWindowFlags.Default, DisplayDevice.Default, 4, 1, GraphicsContextFlags.ForwardCompatible
		) {
			Stopwatch.Start();
			Gui = new Gui(new GuiRenderer()) {
				new Window("Status") {
					new Size(500, 100), 
					new Text(() => $"Position {Camera.Position}"), 
					new Text(() => $"FPS {(FrameTimes.Count == 0 ? 0 : 1 / (FrameTimes.Sum() / (double) FrameTimes.Count))}")
				}
			};
			
			GL.ClearColor(0, 0, 1, 1);

			MouseMove += (_, e) => Gui.MousePosition = (e.X, e.Y);
			MouseDown += (_, e) => UpdateMouseButton(e.Button, true);
			MouseUp += (_, e) => UpdateMouseButton(e.Button, false);
			MouseWheel += (_, e) => Gui.WheelDelta += e.Delta;
		}

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
			var movescale = KeyState.Keys.Contains(Key.WinLeft) ? 150 : 30;
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
			GL.Viewport(0, 0, Width, Height);
			ProjectionMat = Mat4.Perspective(45 * (Math.PI / 180), (double) Width / Height, 1, 5000);
		}

		protected override void OnRenderFrame(FrameEventArgs e) {
			if(FrameTimes.Count == 200)
				FrameTimes.RemoveAt(0);
			FrameTimes.Add(e.Time);
			GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);
			
			GL.Enable(EnableCap.CullFace);
			GL.Enable(EnableCap.DepthTest);
			
			Mesh.SetProjectionView(FpsCamera.Matrix * ProjectionMat);
			
			Models.ForEach(model => model.Draw());

			Gui.Render((float) e.Time);

			SwapBuffers();
		}
	}
}