using System.Collections.Generic;

namespace OpenEQ.NsimGui {
	public enum TextureFormat {
		Alpha,
		Rgb, 
		Rgba
	}

	public struct DrawCommand {
		public int TextureId;
		public (float X, float Y, float Width, float Height) Scissor;
		public uint IndexOffset, ElementCount;
	}

	public struct DrawCommandSet {
		public byte[] VBufferData;
		public ushort[] IBufferData;
		public IReadOnlyList<DrawCommand> Commands;
	}
	
	public interface IGuiRenderer {
		int CreateTexture(TextureFormat format, int width, int height, byte[] pixels);
		void DeleteTexture(int id);
		void Draw(IReadOnlyList<DrawCommandSet> commandsets);
	}
}