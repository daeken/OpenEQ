using System;
using OpenEQ.NsimGui;
using OpenTK;
using OpenTK.Graphics;
using OpenTK.Graphics.OpenGL4;
using Vector2 = System.Numerics.Vector2;

namespace OpenEQ.Engine {
	public class EngineCore : GameWindow {
		public readonly Gui Gui;
		
		public EngineCore() : base(
			1280, 720, GraphicsMode.Default, "OpenEQ", 
			GameWindowFlags.Default, DisplayDevice.Default, 4, 1, GraphicsContextFlags.ForwardCompatible
		) {
			Gui = new Gui(new GuiRenderer());
			
			GL.ClearColor(0, 0, 1, 1);
		}

		protected override void OnResize(EventArgs e) {
			Gui.Dimensions = new Vector2(Width, Height);
			GL.Viewport(0, 0, Width, Height);
		}

		protected override void OnRenderFrame(FrameEventArgs e) {
			GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);
			
			Gui.Render((float) e.Time);
			
			SwapBuffers();
		}
	}
}