using System;
using System.IO;
using static System.Console;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
    public class Texture {
        int id;

        public Texture(Stream data) {
            var reader = new BinaryReader(data);
            data.Position = 12;
            var height = reader.ReadInt32();
            var width = reader.ReadInt32();
            var linearSize = reader.ReadUInt32();
            data.Position += 4;
            var mipMapCount = reader.ReadUInt32();
            data.Position = 84;
            var fourcc = reader.ReadUInt32();
            data.Position = 128;
            var pdata = reader.ReadBytes((int) (mipMapCount > 1 ? linearSize * 2 : linearSize));

            PixelInternalFormat format = PixelInternalFormat.Rgba; // Just to shut up the compiler
            switch(fourcc) {
                case 0x31545844: // DXT1
                    format = PixelInternalFormat.CompressedRgbaS3tcDxt1Ext;
                    break;
                case 0x33545844: // DXT3
                    format = PixelInternalFormat.CompressedRgbaS3tcDxt3Ext;
                    break;
                case 0x35545844: // DXT5
                    format = PixelInternalFormat.CompressedRgbaS3tcDxt5Ext;
                    break;
            }
            
            id = GL.GenTexture();
            GL.ActiveTexture(TextureUnit.Texture0);
            GL.BindTexture(TextureTarget.Texture2D, id);

            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureWrapS, (int) All.Repeat);
            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureWrapT, (int) All.Repeat);
            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMagFilter, (int) All.Nearest);
            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int) All.NearestMipmapNearest);
            GL.TexParameter(TextureTarget.Texture2D, (TextureParameterName) All.TextureMaxAnisotropyExt, 4f);

            GL.CompressedTexImage2D(TextureTarget.Texture2D, 0, format, width, height, 0, (int) linearSize, pdata);
            GL.GenerateMipmap(GenerateMipmapTarget.Texture2D);
        }

        public void Enable() {
            GL.ActiveTexture(TextureUnit.Texture0);
            GL.BindTexture(TextureTarget.Texture2D, id);
        }
    }
}