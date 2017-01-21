using System.IO;
using static System.Console;
using SiliconStudio.Xenko.Engine;
using SiliconStudio.Xenko.Graphics;
using Pfim;
using System.Runtime.InteropServices;
using ImageSharp;
using ImageSharp.Processing;

namespace OpenEQ {
    static class TextureLoader {
        static TextureLoader() {
            ImageSharp.Configuration.Default.AddImageFormat(new ImageSharp.Formats.PngFormat());
        }

        public static Texture Load(Game game, Stream stream, int length) {
            var zfp = new BinaryReader(stream);
            var data = zfp.ReadBytes(length);
            var img = SiliconStudio.Xenko.Graphics.Image.Load(data);
            if(img.Description.MipLevels != SiliconStudio.Xenko.Graphics.Image.CountMips(img.Description.Width, img.Description.Height) && img.Description.Width == img.Description.Height) {
                var timg = MakeMipsImage(data);
                if(timg != null)
                    img = timg;
            }
            WriteLine($"Loading image with length {length}");
            return Texture.New(game.GraphicsDevice, img);
        }

        public unsafe static SiliconStudio.Xenko.Graphics.Image MakeMipsImage(byte[] data) {
            var dds = Dds.Create(new MemoryStream(data));
            if(dds.BytesPerPixel == 4) // No mipmaps for transparent textures.  For now.
                return null;
            var tlen = dds.Width * dds.Height * 2 * 4;
            var mips = SiliconStudio.Xenko.Graphics.Image.CountMips(dds.Width, dds.Height);
            var adata = Marshal.AllocHGlobal(tlen);

            var isi = ISImageFromDDS(dds);

            var off = 0;
            for(var i = 0; i < mips; ++i)
                off += MakeMipLevel(isi, (uint*) adata, i, off);

            return SiliconStudio.Xenko.Graphics.Image.New2D(dds.Width, dds.Height, mips, PixelFormat.R8G8B8A8_UNorm, 1, adata);
        }

        static ImageSharp.Image ISImageFromDDS(Dds dds) {
            var isi = new ImageSharp.Image(dds.Width, dds.Height);
            isi.SetPixels(dds.Width, dds.Height, DataToColors(dds.Data, dds.BytesPerPixel));
            return isi;
        }

        static Color[] DataToColors(byte[] data, int bpp) {
            var colors = new Color[data.Length / bpp];
            var off = 0;
            for(var i = 0; i < colors.Length; ++i) {
                var r = data[off + 2];
                var g = data[off + 1];
                var b = data[off + 0];
                off += 3;
                var a = bpp == 4 ? data[off++] : (byte) 255;
                colors[i] = new Color(r, g, b, a);
            }
            return colors;
        }

        static unsafe int CopyColorsToPtr(Color[] colors, uint* odata, int offset) {
            for(var i = 0; i < colors.Length; ++i)
                odata[i + offset] = colors[i].PackedValue;
            return colors.Length;
        }

        static unsafe int MakeMipLevel(ImageSharp.Image isi, uint* odata, int miplevel, int offset) {
            return CopyColorsToPtr((miplevel == 0 ? isi : isi.Resize(isi.Width >> 1, isi.Height >> 1, new Lanczos2Resampler(), true)).Pixels, odata, offset);
        }
    }
}
