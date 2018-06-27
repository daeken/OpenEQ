using System;
using System.Numerics;
using ImGuiNET;

namespace OpenEQ.NsimGui {
	public class Gui {
		static IO IO => ImGui.GetIO();

		readonly IGuiRenderer Renderer;
		
		public Vector2 Dimensions {
			get => IO.DisplaySize;
			set => IO.DisplaySize = value;
		}
		
		public Vector2 Scale {
			get => IO.DisplayFramebufferScale;
			set => IO.DisplaySize = value;
		}
		
		public unsafe Gui(IGuiRenderer renderer) {
			Renderer = renderer;
			IO.FontAtlas.AddDefaultFont();
			var fontTex = IO.FontAtlas.GetTexDataAsAlpha8();
			var pixels = new byte[fontTex.Width * fontTex.Height];
			for(var i = 0; i < pixels.Length; ++i)
				pixels[i] = fontTex.Pixels[i];
			Renderer.CreateTexture(TextureFormat.Alpha, fontTex.Width, fontTex.Height, pixels);
		}

		public void Render(float deltaTime) {
			IO.DeltaTime = deltaTime;
			
			ImGui.NewFrame();
			
			ImGui.Render();
			
			Renderer.Draw();
		}
	}
}