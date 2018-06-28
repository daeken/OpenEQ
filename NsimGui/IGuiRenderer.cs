using System.Collections.Generic;

namespace NsimGui {
	public enum TextureFormat {
		Alpha,
		Rgb, 
		Rgba
	}

	public struct DrawCommand {
		public int TextureId;
		public (int X, int Y, int Width, int Height) Scissor;
		public int IndexOffset, ElementCount;
	}

	public struct DrawCommandSet {
		public byte[] VBufferData;
		public ushort[] IBufferData;
		public IReadOnlyList<DrawCommand> Commands;
	}
	
	public interface IGuiRenderer {
		int CreateTexture(TextureFormat format, int width, int height, byte[] pixels);
		void DeleteTexture(int id);
		void Draw((float, float) dimensions, IReadOnlyList<DrawCommandSet> commandsets);
	}
}