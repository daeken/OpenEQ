using System;
using static System.Console;
using OpenTK.Graphics.OpenGL4;
using LibRocketNet;
using OpenEQ.Engine;
using System.Runtime.InteropServices;
using System.IO;
using System.Drawing;

namespace OpenEQ.GUI {
    struct RocketGeometry {
        public int vao;
        public int vbo;
        public int ibo;
        public int indexCount;
        public int texId;
    }

    public class OEQRenderInterface : RenderInterface {
        CoreEngine engine;
        int width, height;
        int shader, uniformProjMtx, uniformTranslation, attribPosition, attribUV, attribColor;

        int whiteTexture;

        public OEQRenderInterface(CoreEngine engine) {
            this.engine = engine;

            var vsSource = File.ReadAllText("shaders/overlayvert.glsl");
            var fsSource = File.ReadAllText("shaders/overlayfrag.glsl");

            shader = GL.CreateProgram();
            var vertHandle = GL.CreateShader(ShaderType.VertexShader);
            var fragHandle = GL.CreateShader(ShaderType.FragmentShader);
            GL.ShaderSource(vertHandle, vsSource);
            GL.ShaderSource(fragHandle, fsSource);
            GL.CompileShader(vertHandle);
            GL.CompileShader(fragHandle);
            GL.AttachShader(shader, vertHandle);
            GL.AttachShader(shader, fragHandle);
            GL.LinkProgram(shader);
            GL.UseProgram(shader);

            uniformProjMtx = GL.GetUniformLocation(shader, "Projection");
            uniformTranslation = GL.GetUniformLocation(shader, "Translation");
            attribPosition = GL.GetAttribLocation(shader, "Position");
            attribUV = GL.GetAttribLocation(shader, "UV");
            attribColor = GL.GetAttribLocation(shader, "Color");

            var temp = IntPtr.Zero;
            MakeTexture(ref temp, new byte[] { 255, 255, 255, 255 }, new Vector2i(1, 1), false);
            whiteTexture = temp.ToInt32();
        }

        public void Resize(int width, int height) {
            this.width = width;
            this.height = height;
        }

        protected override void EnableScissorRegion(bool enable) {
            if(enable)
                GL.Enable(EnableCap.ScissorTest);
            else
                GL.Disable(EnableCap.ScissorTest);
        }

        protected override unsafe void RenderGeometry(Vertex* vertices, int num_vertices, int* indices, int num_indices, IntPtr texture, Vector2f translation) {
            var geom = CompileGeometry(vertices, num_vertices, indices, num_indices, texture, temporary: true);
            RenderCompiledGeometry(geom, translation);
            ReleaseCompiledGeometry(geom);
        }

        protected override void SetScissorRegion(int x, int y, int width, int height) {
            GL.Scissor((int) (x * engine.DisplayScale), (int) ((this.height - (y + height)) * engine.DisplayScale), (int) (width * engine.DisplayScale), (int) (height * engine.DisplayScale));
        }

        protected override unsafe IntPtr CompileGeometry(Vertex* vertices, int num_vertices, int* indices, int num_indices, IntPtr texture) {
            return CompileGeometry(vertices, num_vertices, indices, num_indices, texture, temporary: false);
        }

        unsafe IntPtr CompileGeometry(Vertex* vertices, int num_vertices, int* indices, int num_indices, IntPtr texture, bool temporary = false) {
            var geom = new RocketGeometry();
            geom.indexCount = num_indices;
            geom.texId = texture.ToInt32();
            if(geom.texId == 0)
                geom.texId = whiteTexture;

            geom.vao = GL.GenVertexArray();
            GL.BindVertexArray(geom.vao);

            var vertexBuffer = geom.vbo = GL.GenBuffer();
            GL.BindBuffer(BufferTarget.ArrayBuffer, vertexBuffer);
            GL.BufferData(BufferTarget.ArrayBuffer, num_vertices * sizeof(Vertex), new IntPtr(vertices), temporary ? BufferUsageHint.StreamDraw : BufferUsageHint.StaticDraw);

            GL.EnableVertexAttribArray(attribPosition);
            GL.EnableVertexAttribArray(attribUV);
            GL.EnableVertexAttribArray(attribColor);
            GL.VertexAttribPointer(attribPosition, 2, VertexAttribPointerType.Float, false, sizeof(Vertex), 0); // Position
            GL.VertexAttribPointer(attribUV, 2, VertexAttribPointerType.Float, false, sizeof(Vertex), 3 * sizeof(float)); // Texcoord
            GL.VertexAttribPointer(attribColor, 4, VertexAttribPointerType.UnsignedByte, true, sizeof(Vertex), 2 * sizeof(float)); // Color

            var indexBuffer = geom.ibo = GL.GenBuffer();
            GL.BindBuffer(BufferTarget.ElementArrayBuffer, indexBuffer);
            GL.BufferData(BufferTarget.ElementArrayBuffer, num_indices * sizeof(uint), new IntPtr(indices), temporary ? BufferUsageHint.StreamDraw : BufferUsageHint.StaticDraw);

            GL.BindVertexArray(0);

            var ptr = Marshal.AllocHGlobal(sizeof(RocketGeometry));
            Marshal.StructureToPtr(geom, ptr, false);
            return ptr;
        }

        protected override unsafe void ReleaseCompiledGeometry(IntPtr geometry) {
            var geom = (RocketGeometry*) geometry.ToPointer();
            GL.DeleteBuffer(geom->vbo);
            GL.DeleteBuffer(geom->ibo);
            GL.DeleteVertexArray(geom->vao);
        }

        public void Render(Context context) {
            GL.UseProgram(shader);
            var ortho = new float[] {
                2.0f / width, 0, 0, 0,
                0, 2.0f / -height, 0, 0,
                0, 0, -1, 0,
                -1, 1, 0, 1
            };
            GL.UniformMatrix4(uniformProjMtx, 1, false, ortho);

            GL.Disable(EnableCap.CullFace);
            GL.Disable(EnableCap.DepthTest);
            
            context.Render();

            GL.Enable(EnableCap.CullFace);
            GL.Enable(EnableCap.DepthTest);

            GL.UseProgram(0);
        }

        protected override unsafe void RenderCompiledGeometry(IntPtr geometry, Vector2f translation) {
            GL.Uniform2(uniformTranslation, new OpenTK.Vector2(translation.X, translation.Y));

            var geom = (RocketGeometry*) geometry.ToPointer();
            GL.BindTexture(TextureTarget.Texture2D, geom->texId);
            GL.BindVertexArray(geom->vao);
            GL.DrawElements(BeginMode.Triangles, geom->indexCount, DrawElementsType.UnsignedInt, 0);
            GL.BindVertexArray(0);
        }

        protected override unsafe bool LoadTexture(ref IntPtr texture_handle, ref Vector2i texture_dimensions, string source) {
            if(source.EndsWith(".tga")) {
                var img = Pfim.Pfim.FromFile(source);
                texture_dimensions.X = img.Width;
                texture_dimensions.Y = img.Height;
                return MakeTexture(ref texture_handle, img.Data, texture_dimensions, true);
            } else {
                var img = new Bitmap(source);
                texture_dimensions.X = img.Width;
                texture_dimensions.Y = img.Height;
                var locked = img.LockBits(new Rectangle(0, 0, img.Width, img.Height), System.Drawing.Imaging.ImageLockMode.ReadOnly, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
                var success = MakeTexture(ref texture_handle, locked.Scan0, texture_dimensions, true);
                img.UnlockBits(locked);
                return success;
            }
        }

        protected override unsafe bool GenerateTexture(ref IntPtr texture_handle, byte* source, Vector2i source_dimensions) {
            return MakeTexture(ref texture_handle, new IntPtr(source), source_dimensions, false);
        }

        bool MakeTexture<T>(ref IntPtr texture_handle, T source, Vector2i source_dimensions, bool reverse) {
            var id = GL.GenTexture();
            GL.ActiveTexture(TextureUnit.Texture0);
            GL.BindTexture(TextureTarget.Texture2D, id);

            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureWrapS, (int) All.ClampToEdge);
            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureWrapT, (int) All.ClampToEdge);
            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMagFilter, (int) All.Linear);
            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int) All.Linear);
            if(source is IntPtr)
                GL.TexImage2D(TextureTarget.Texture2D, 0, PixelInternalFormat.Rgba, source_dimensions.X, source_dimensions.Y, 0, reverse ? PixelFormat.Bgra : PixelFormat.Rgba, PixelType.UnsignedByte, (IntPtr) (object) source);
            else
                GL.TexImage2D(TextureTarget.Texture2D, 0, PixelInternalFormat.Rgba, source_dimensions.X, source_dimensions.Y, 0, reverse ? PixelFormat.Bgra : PixelFormat.Rgba, PixelType.UnsignedByte, (byte[]) (object) source);

            texture_handle = new IntPtr(id);
            return true;
        }

        protected override void ReleaseTexture(IntPtr texptr) {
            GL.DeleteTexture(texptr.ToInt32());
        }
    }
}
