using System;
using System.Numerics;
using static System.Console;

using OpenEQ.Engine;
using OpenTK.Graphics.OpenGL4;
using ImGuiNET;
using System.IO;
using System.Diagnostics;
using OpenTK;
using OpenTK.Input;
using System.Drawing;
using System.Collections.Generic;

namespace OpenEQ.GUI {
    public class CoreGUI {
        CoreEngine engine;
        GameWindow window;
        int width, height;
        float scale;
        float lastTime;

        bool loaded = false;
        int shader, vertHandle, fragHandle;
        int attribTex, attribProjMtx, attribPosition, attribUV, attribColor;
        int vbo, elements, vao, fontTexture;

        List<Window> windows;

        public CoreGUI(CoreEngine _engine, GameWindow _window, float graphicsScale) {
            windows = new List<Window>();
            engine = _engine;
            window = _window;
            scale = graphicsScale;
            ImGui.LoadDefaultFont();
            MapInput();
        }

        public void Add(Window window) {
            windows.Add(window);
        }

        unsafe bool WantKeyboard {
            get {
                var np = ImGui.GetIO().GetNativePointer();
                return np->WantCaptureKeyboard != 0;
            }
        }

        public unsafe bool WantMouse {
            get {
                var np = ImGui.GetIO().GetNativePointer();
                return np->WantCaptureMouse != 0;
            }
        }

        void MapInput() {
            var io = ImGui.GetIO();
            io.KeyMap[GuiKey.Tab] = (int)Key.Tab;
            io.KeyMap[GuiKey.LeftArrow] = (int)Key.Left;
            io.KeyMap[GuiKey.RightArrow] = (int)Key.Right;
            io.KeyMap[GuiKey.UpArrow] = (int)Key.Up;
            io.KeyMap[GuiKey.DownArrow] = (int)Key.Down;
            io.KeyMap[GuiKey.PageUp] = (int)Key.PageUp;
            io.KeyMap[GuiKey.PageDown] = (int)Key.PageDown;
            io.KeyMap[GuiKey.Home] = (int)Key.Home;
            io.KeyMap[GuiKey.End] = (int)Key.End;
            io.KeyMap[GuiKey.Delete] = (int)Key.Delete;
            io.KeyMap[GuiKey.Backspace] = (int)Key.BackSpace;
            io.KeyMap[GuiKey.Enter] = (int)Key.Enter;
            io.KeyMap[GuiKey.Escape] = (int)Key.Escape;
            io.KeyMap[GuiKey.A] = (int)Key.A;
            io.KeyMap[GuiKey.C] = (int)Key.C;
            io.KeyMap[GuiKey.V] = (int)Key.V;
            io.KeyMap[GuiKey.X] = (int)Key.X;
            io.KeyMap[GuiKey.Y] = (int)Key.Y;
            io.KeyMap[GuiKey.Z] = (int)Key.Z;
            
            window.KeyDown += (sender, e) => {
                if(WantKeyboard)
                    ImGui.GetIO().KeysDown[(int)e.Key] = true;
                else
                    engine.KeyDown(e);
            };
            window.KeyUp += (sender, e) => {
                if(WantKeyboard)
                    ImGui.GetIO().KeysDown[(int)e.Key] = false;
                else
                    engine.KeyUp(e);
            };
            window.KeyPress += (sender, e) => {
                ImGui.AddInputCharacter(e.KeyChar);
            };
        }

        unsafe void CreateDeviceObjects() {
            var lastTexture = GL.GetInteger(GetPName.TextureBinding2D);
            var lastArrayBuffer = GL.GetInteger(GetPName.ArrayBufferBinding);
            var lastVertexArray = GL.GetInteger(GetPName.VertexArrayBinding);
            
            var vsSource = File.ReadAllText("shaders/overlayvert.glsl");
            var fsSource = File.ReadAllText("shaders/overlayfrag.glsl");

            shader = GL.CreateProgram();
            vertHandle = GL.CreateShader(ShaderType.VertexShader);
            fragHandle = GL.CreateShader(ShaderType.FragmentShader);
            GL.ShaderSource(vertHandle, vsSource);
            GL.ShaderSource(fragHandle, fsSource);
            GL.CompileShader(vertHandle);
            GL.CompileShader(fragHandle);
            GL.AttachShader(shader, vertHandle);
            GL.AttachShader(shader, fragHandle);
            GL.LinkProgram(shader);

            Debug.Assert(GL.GetError() == ErrorCode.NoError);
            attribTex = GL.GetUniformLocation(shader, "Tex");
            attribProjMtx = GL.GetUniformLocation(shader, "Projection");
            attribPosition = GL.GetAttribLocation(shader, "Position");
            attribUV = GL.GetAttribLocation(shader, "UV");
            attribColor = GL.GetAttribLocation(shader, "Color");

            vbo = GL.GenBuffer();
            elements = GL.GenBuffer();
            vao = GL.GenVertexArray();
            GL.BindVertexArray(vao);
            GL.BindBuffer(BufferTarget.ArrayBuffer, vbo);
            GL.EnableVertexAttribArray(attribPosition);
            GL.EnableVertexAttribArray(attribUV);
            GL.EnableVertexAttribArray(attribColor);

            GL.VertexAttribPointer(attribPosition, 2, VertexAttribPointerType.Float, false, sizeof(DrawVert), DrawVert.PosOffset);
            GL.VertexAttribPointer(attribUV, 2, VertexAttribPointerType.Float, false, sizeof(DrawVert), DrawVert.UVOffset);
            GL.VertexAttribPointer(attribColor, 4, VertexAttribPointerType.UnsignedByte, true, sizeof(DrawVert), DrawVert.ColOffset);

            CreateFontsTexture();

            GL.BindTexture(TextureTarget.Texture2D, lastTexture);
            GL.BindBuffer(BufferTarget.ArrayBuffer, lastArrayBuffer);
            GL.BindVertexArray(lastVertexArray);
        }

        unsafe void CreateFontsTexture() {
            var io = ImGui.GetIO();
            var tdata = io.FontAtlas.GetTexDataAsRGBA32();

            fontTexture = GL.GenTexture();
            GL.BindTexture(TextureTarget.Texture2D, fontTexture);
            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int)All.Linear);
            GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMagFilter, (int)All.Linear);
            GL.TexImage2D(
                TextureTarget.Texture2D, 
                0, 
                PixelInternalFormat.Rgba, 
                tdata.Width, 
                tdata.Height, 
                0, 
                PixelFormat.Rgba, 
                PixelType.UnsignedByte, 
                new IntPtr(tdata.Pixels)
            );

            io.FontAtlas.SetTexID(fontTexture);
            io.FontAtlas.ClearTexData();
        }

        public void Resize(int width, int height) {
            this.width = width;
            this.height = height;
        }

        public void Render() {
            var io = ImGui.GetIO();
            io.DisplaySize = new System.Numerics.Vector2(width, height);
            io.DisplayFramebufferScale = new System.Numerics.Vector2(scale);

            var curtime = Time.Now;
            io.DeltaTime = lastTime == 0 ? 1.0f/60 : (curtime - lastTime);
            lastTime = curtime;

            if(!loaded) {
                CreateDeviceObjects();
                loaded = true;
            }

            UpdateInput();

            ImGui.NewFrame();

            /*ImGui.GetStyle().WindowRounding = 10;
            ImGui.SetNextWindowSize(new System.Numerics.Vector2(600, 400), SetCondition.FirstUseEver);
            bool _mainWindowOpened = true;
            ImGui.BeginWindow("OpenEQ Testing", ref _mainWindowOpened, WindowFlags.Default);
            ImGuiNative.igSetWindowFontScale(2.0f);
            if(ImGui.Button("Foo!", new System.Numerics.Vector2(200, 100)))
                WriteLine("adpsofj");
            ImGui.Text("adfjpoj");
            ImGui.EndWindow();

            ImGui.BeginWindow("OpenEQ Testing 2", ref _mainWindowOpened, WindowFlags.Default);
            ImGuiNative.igSetWindowFontScale(2.0f);
            if(ImGui.Button("Foo!", new System.Numerics.Vector2(200, 100)))
                WriteLine("adpsofj");
            ImGui.Text("adfjpoj");
            ImGui.EndWindow();*/
            foreach(var window in windows)
                window.Render();

            ImGui.Render();

            Draw(io);
        }

        void UpdateInput() {
            var io = ImGui.GetIO();
            var cstate = Mouse.GetCursorState();
            var mstate = Mouse.GetState();

            if(!engine.MouseLook && window.Bounds.Contains(cstate.X, cstate.Y)) {
                var wpoint = window.PointToClient(new Point(cstate.X, cstate.Y));
                io.MousePosition = new System.Numerics.Vector2(wpoint.X / io.DisplayFramebufferScale.X, wpoint.Y / io.DisplayFramebufferScale.Y);
            } else {
                io.MousePosition = new System.Numerics.Vector2(-1, -1);
            }

            io.MouseDown[0] = mstate.LeftButton == ButtonState.Pressed;
            io.MouseDown[1] = mstate.RightButton == ButtonState.Pressed;
            io.MouseDown[2] = mstate.MiddleButton == ButtonState.Pressed;
        }

        unsafe void Draw(IO io) {
            var dd = ImGui.GetDrawData();
            ImGui.ScaleClipRects(dd, io.DisplayFramebufferScale);

            var lastProgram = GL.GetInteger(GetPName.CurrentProgram);
            var lastTexture = GL.GetInteger(GetPName.TextureBinding2D);
            var lastActiveTex = GL.GetInteger(GetPName.ActiveTexture);
            var lastArrayBuffer = GL.GetInteger(GetPName.ArrayBufferBinding);
            var lastElementArray = GL.GetInteger(GetPName.ElementArrayBufferBinding);
            var lastVertexArray = GL.GetInteger(GetPName.VertexArrayBinding);
            var lastBlendSrc = GL.GetInteger(GetPName.BlendSrc);
            var lastBlendDst = GL.GetInteger(GetPName.BlendDst);
            var lastBlendEqRgb = GL.GetInteger(GetPName.BlendEquationRgb);
            var lastBlendEqAlpha = GL.GetInteger(GetPName.BlendEquationAlpha);
            var lastViewport = new int[4];
            GL.GetInteger(GetPName.Viewport, lastViewport);
            var lastScissorBox = new int[4];
            GL.GetInteger(GetPName.ScissorBox, lastScissorBox);
            var lastEBlend = GL.IsEnabled(EnableCap.Blend);
            var lastECull = GL.IsEnabled(EnableCap.CullFace);
            var lastEDepthTest = GL.IsEnabled(EnableCap.DepthTest);
            var lastEScissorTest = GL.IsEnabled(EnableCap.ScissorTest);

            GL.Enable(EnableCap.Blend);
            GL.BlendFunc(BlendingFactorSrc.SrcAlpha, BlendingFactorDest.OneMinusSrcAlpha);
            GL.Disable(EnableCap.CullFace);
            GL.Disable(EnableCap.DepthTest);
            GL.Enable(EnableCap.ScissorTest);
            GL.ActiveTexture(TextureUnit.Texture0);

            var ortho = new float[] {
                2.0f / width * scale, 0, 0, 0, 
                0, 2.0f / -height * scale, 0, 0, 
                0, 0, -1, 0, 
                -1, 1, 0, 1
            };

            GL.Viewport(0, 0, (int) (width * scale), (int) (height * scale));
            GL.UseProgram(shader);
            GL.Uniform1(attribTex, 0);
            GL.UniformMatrix4(attribProjMtx, 1, false, ortho);
            GL.BindVertexArray(vao);

            for(var n = 0; n < dd->CmdListsCount; ++n) {
                var cmdlist = dd->CmdLists[n];
                var off = 0;

                GL.BindBuffer(BufferTarget.ArrayBuffer, vbo);
                GL.BufferData(BufferTarget.ArrayBuffer, cmdlist->VtxBuffer.Size * sizeof(DrawVert), new IntPtr(cmdlist->VtxBuffer.Data), BufferUsageHint.StreamDraw);
                GL.BindBuffer(BufferTarget.ElementArrayBuffer, elements);
                GL.BufferData(BufferTarget.ElementArrayBuffer, cmdlist->IdxBuffer.Size * 2, new IntPtr(cmdlist->IdxBuffer.Data), BufferUsageHint.StreamDraw);
                
                for(var i = 0; i < cmdlist->CmdBuffer.Size; ++i) {
                    var pcmd = &(((DrawCmd *) cmdlist->CmdBuffer.Data)[i]);
                    GL.BindTexture(TextureTarget.Texture2D, pcmd->TextureId.ToInt32());
                    GL.Scissor(
                        (int) pcmd->ClipRect.X, 
                        (int) (io.DisplaySize.Y - pcmd->ClipRect.W), 
                        (int) (pcmd->ClipRect.Z - pcmd->ClipRect.X), 
                        (int) (pcmd->ClipRect.W - pcmd->ClipRect.Y)
                    );
                    GL.DrawElements(BeginMode.Triangles, (int) pcmd->ElemCount, DrawElementsType.UnsignedShort, off * 2);
                    off += (int) pcmd->ElemCount;
                }
            }
            
            GL.UseProgram(lastProgram);
            GL.ActiveTexture((TextureUnit) lastActiveTex);
            GL.BindTexture(TextureTarget.Texture2D, lastTexture);
            GL.BindVertexArray(lastVertexArray);
            GL.BindBuffer(BufferTarget.ArrayBuffer, lastArrayBuffer);
            GL.BindBuffer(BufferTarget.ElementArrayBuffer, lastElementArray);
            GL.BlendEquationSeparate((BlendEquationMode) lastBlendEqRgb, (BlendEquationMode) lastBlendEqAlpha);
            GL.BlendFunc((BlendingFactorSrc) lastBlendSrc, (BlendingFactorDest) lastBlendDst);
            if(lastEBlend) GL.Enable(EnableCap.Blend); else GL.Disable(EnableCap.Blend);
            if(lastECull) GL.Enable(EnableCap.CullFace); else GL.Disable(EnableCap.CullFace);
            if(lastEDepthTest) GL.Enable(EnableCap.DepthTest); else GL.Disable(EnableCap.DepthTest);
            if(lastEScissorTest) GL.Enable(EnableCap.ScissorTest); else GL.Disable(EnableCap.ScissorTest);
            GL.Viewport(lastViewport[0], lastViewport[1], lastViewport[2], lastViewport[3]);
            GL.Scissor(lastScissorBox[0], lastScissorBox[1], lastScissorBox[2], lastScissorBox[3]);
        }
    }
}
