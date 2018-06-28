using System;
using System.Linq;
using NsimGui;
using NsimGui.Widgets;
using OpenTK;
using OpenTK.Graphics;
using OpenTK.Graphics.OpenGL4;
using OpenTK.Input;
using Vector2 = System.Numerics.Vector2;

namespace OpenEQ.Engine {
	public class EngineCore : GameWindow {
		public readonly Gui Gui;
		
		public EngineCore() : base(
			1280, 720, GraphicsMode.Default, "OpenEQ", 
			GameWindowFlags.Default, DisplayDevice.Default, 4, 1, GraphicsContextFlags.ForwardCompatible
		) {
			var counter = 0;
			Gui = new Gui(new GuiRenderer()) {
				new Window(() => $"Testing! {counter}") {
					new Button("Foo") { _ => counter++ }, 
					new Button("Bar")
				}, 
				new Window("Testing 2!") {
					new Text("Testing testing testing"),
					new Button("Blah") { _ => ((Window) Gui.First()).Add(new Button($"Blah {counter}")) }, 
					new Text(() => $"Some more text -- { counter }")
				}
			};
			
			GL.ClearColor(0, 0, 1, 1);

			MouseMove += (_, e) => Gui.MousePosition = (e.X, e.Y);
			MouseDown += (_, e) => UpdateMouseButton(e.Button, true);
			MouseUp += (_, e) => UpdateMouseButton(e.Button, false);
			MouseWheel += (_, e) => Gui.WheelDelta += e.Delta;
		}

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

		protected override void OnResize(EventArgs e) {
			Gui.Dimensions = new Vector2(Width, Height);
			Gui.Scale = new Vector2(2f);
			GL.Viewport(0, 0, Width, Height);
		}

		protected override void OnRenderFrame(FrameEventArgs e) {
			GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);

			Gui.Render((float) e.Time);

			SwapBuffers();
		}
	}
}