using OpenTK;
using OpenTK.Graphics;
using OpenTK.Graphics.OpenGL4;
using System;
using static System.Console;
using System.Collections.Generic;
using OpenTK.Input;
using System.IO;

namespace OpenEQ.Engine {
    public class CoreEngine {
        GameWindow window;

        Program program;
        List<Placeable> placeables;
        Matrix4 perspective;

        Camera camera;
        Vector3 movement;
        Vector2 mouselast, keylook;
        bool mouselook = false;

        public CoreEngine() {
            placeables = new List<Placeable>();
            window = new GameWindow(1280, 720, new GraphicsMode(new ColorFormat(32), 32), "OpenEQ", GameWindowFlags.Default, null, 4, 1, GraphicsContextFlags.ForwardCompatible);

            camera = new Camera(new Vector3(100, 0, 0));
            perspective = Matrix4.CreatePerspectiveFieldOfView((float) (60 * Math.PI / 180), ((float) window.Width) / window.Height, 1, 10000);

            movement = new Vector3();
            keylook = new Vector2();
            
            window.Load += (sender, e) => Load();
            window.Resize += (sender, e) => Resize();
            window.UpdateFrame += (sender, e) => Update();
            window.RenderFrame += (sender, e) => Render();
            window.KeyDown += (sender, e) => KeyDown(e);
            window.KeyUp += (sender, e) => KeyUp(e);
            /*window.MouseDown += (sender, e) => {
                if(e.Button != MouseButton.Right)
                    return;
                mouselook = true;
                mouselast = new Vector2(-1, -1);
            };
            window.MouseUp += (sender, e) => {
                if(e.Button != MouseButton.Right)
                    return;
                mouselook = false;
            };*/
        }

        public void AddPlaceable(Placeable placeable) {
            placeables.Add(placeable);
        }

        void Load() {
            GL.ClearColor(Color.MidnightBlue);
            
            var vertshader = new Shader(File.ReadAllText("shaders/basevert.glsl"), ShaderType.VertexShader);
            var fragshader = new Shader(File.ReadAllText("shaders/basefrag.glsl"), ShaderType.FragmentShader);
            program = new Program(vertshader, fragshader);

            GL.CullFace(CullFaceMode.Back);
            GL.FrontFace(FrontFaceDirection.Cw);
            GL.Enable(EnableCap.CullFace);
        }

        void Resize() {
            WriteLine($"resizing {window.Width}x{window.Height}");
            GL.Viewport(0, 0, window.Width, window.Height);
        }

        void Render() {
            GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);
            GL.Enable(EnableCap.DepthTest);

            program.Use();
            var vp = camera.Matrix * perspective;
            foreach(var placeable in placeables) {
                var model = placeable.Mat;
                var mvp = model * vp;
                var mv = model * camera.Matrix;
                program.Uniform("MVPMatrix", mvp);
                program.Uniform("MVMatrix", mv);
                placeable.Draw();
            }

            window.SwapBuffers();
        }

        void Update() {
            if(movement.Length != 0)
                camera.Translate(movement * 2);
            if(keylook.Length != 0)
                camera.Rotate(keylook / 50);
            // if(mouselook) {
            //     var mouse = Mouse.GetState();
            //     WriteLine($"{mouse.X}, {mouse.Y}");
            //     return;
            //     camera.Rotate(new Vector2(mouse.X, mouse.Y) - mouselast);
            //     var middle = new Vector2(window.Bounds.Left + window.Bounds.Width / 2, window.Bounds.Top + window.Bounds.Height / 2);
            //     mouselast = middle;
            //     Mouse.SetPosition(middle.X, middle.Y);
            //     mouse = Mouse.GetState();
            //     WriteLine($"{window.Bounds.Left} {window.Bounds.Top}");
            // }
        }

        void KeyDown(KeyboardKeyEventArgs e) {
            if(e.IsRepeat) return;
            switch(e.Key) {
                case Key.W:
                    movement.Y += 1;
                    break;
                case Key.S:
                    movement.Y -= 1;
                    break;
                case Key.A:
                    movement.X -= 1;
                    break;
                case Key.D:
                    movement.X += 1;
                    break;
                case Key.Space:
                    movement.Z += 1;
                    break;
                case Key.ControlLeft:
                    movement.Z -= 1;
                    break;
                case Key.Up:
                    keylook.Y -= 1;
                    break;
                case Key.Down:
                    keylook.Y += 1;
                    break;
                case Key.Left:
                    keylook.X -= 1;
                    break;
                case Key.Right:
                    keylook.X += 1;
                    break;
            }
        }
        void KeyUp(KeyboardKeyEventArgs e) {
            switch(e.Key) {
                case Key.W:
                    movement.Y -= 1;
                    break;
                case Key.S:
                    movement.Y += 1;
                    break;
                case Key.A:
                    movement.X += 1;
                    break;
                case Key.D:
                    movement.X -= 1;
                    break;
                case Key.Space:
                    movement.Z -= 1;
                    break;
                case Key.ControlLeft:
                    movement.Z += 1;
                    break;
                case Key.Up:
                    keylook.Y += 1;
                    break;
                case Key.Down:
                    keylook.Y -= 1;
                    break;
                case Key.Left:
                    keylook.X += 1;
                    break;
                case Key.Right:
                    keylook.X -= 1;
                    break;
            }
        }

        public void Run() {
            window.Run(60, 60);
        }
    }
}