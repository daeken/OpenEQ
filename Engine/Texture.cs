using System;
using System.IO;
using static System.Console;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
    public class Texture {
        int id;

        public Texture(Stream data) {
            /*id = GL.GenTexture();
            GL.ActiveTexture(TextureUnit.Texture0);
            GL.BindTexture(TextureTarget.Texture2D, id);
            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureWrapS, (int) All.ClampToEdge);
            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureWrapT, (int) All.ClampToEdge);
            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMagFilter, (int) All.Linear);
            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int) All.Linear);

            var temp = new byte[256*256*3];
            for(var i = 0; i < 256*256*3; i += 3) {
                temp[0] = 255;
                temp[1] = (byte) (i & 0xFF);
                temp[2] = 0;
            }
            //GL.ActiveTexture(TextureUnit.Texture0);
            //GL.BindTexture(TextureTarget.Texture2D, id);
            GL.TexImage2D(TextureTarget.Texture2D, 0, PixelInternalFormat.Rgba8, 256, 256, 0, PixelFormat.Rgb, PixelType.UnsignedByte, temp);
            //GL.GenerateMipmap(GenerateMipmapTarget.Texture2D);

            return;*/
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
            var blockSize = 16;
            switch(fourcc) {
                case 0x31545844: // DXT1
                    format = PixelInternalFormat.CompressedRgbaS3tcDxt1Ext;
                    blockSize = 8;
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
            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int) All.Nearest);

            var offset = 0;
            for(var level = 0; level <= mipMapCount && (width != 0 || height != 0); ++level) {
                var size = ((width + 3) / 4) * ((height + 3) / 4) * blockSize;
                var sub = new byte[size];
                Array.Copy(pdata, offset, sub, 0, size);
                GL.CompressedTexImage2D(TextureTarget.Texture2D, level, format, width, height, 0, size, sub);
                offset += size;
                width /= 2;
                height /= 2;
            }
        }

        public void Enable() {
            GL.ActiveTexture(TextureUnit.Texture0);
            GL.BindTexture(TextureTarget.Texture2D, id);
        }
    }
}