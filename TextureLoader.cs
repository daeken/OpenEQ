using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using Godot;
using System.IO;

class TextureLoader {
	public static Texture Load(Stream data, int size, bool mipmap) {
		var br = new BinaryReader(data);
		var width = br.ReadInt32();
		var height = br.ReadInt32();
		var im = new Image();
		im.CreateFromData(width, height, false, Image.FORMAT_RGBA8, br.ReadBytes(width * height * 4));
		var tex = new ImageTexture();
		tex.CreateFromImage(im, mipmap ? 7 : 6);
		return tex;
	}
}
