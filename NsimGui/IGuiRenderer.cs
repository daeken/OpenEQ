namespace OpenEQ.NsimGui {
	public enum TextureFormat {
		Alpha,
		Rgb, 
		Rgba
	}
	
	public interface IGuiRenderer {
		int CreateTexture(TextureFormat format, int width, int height, byte[] pixels);
		void DeleteTexture(int id);
		void Draw(); // TODO: Make structure for this
	}
}