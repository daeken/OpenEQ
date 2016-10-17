using OpenTK;
using OpenTK.Graphics;
using OpenTK.Graphics.OpenGL4;
using System;
using static System.Console;
using System.Collections.Generic;
using System.Linq;
using OpenTK.Input;
using System.IO;
using System.Drawing;

using OpenEQ.GUI;

namespace OpenEQ.Engine {
    public class CoreEngine {
        public static Queue<Action> GLTaskQueue = new Queue<Action>();

        public CoreGUI Gui;

        GameWindow window;

        Program program;
        List<Placeable> placeables;
        Matrix4 perspective;

        Camera camera;
        Vector3 movement;
        Vector2 mouselast, keylook;
        public bool MouseLook = false;

        public CoreEngine() {
            placeables = new List<Placeable>();
            window = new GameWindow(1280, 720, new GraphicsMode(new ColorFormat(32), 32), "OpenEQ", GameWindowFlags.Default, null, 4, 1, GraphicsContextFlags.ForwardCompatible);

            Gui = new CoreGUI(this, window, window.Width / 1280);

            camera = new Camera(new Vector3(100, 0, 0));
            perspective = Matrix4.CreatePerspectiveFieldOfView((float) (60 * Math.PI / 180), ((float) window.Width) / window.Height, 1, 10000);

            movement = new Vector3();
            keylook = new Vector2();
            
            window.Load += (sender, e) => Load();
            window.Resize += (sender, e) => Resize();
            window.UpdateFrame += (sender, e) => Update();
            window.RenderFrame += (sender, e) => Render();
            window.MouseDown += (sender, e) => {
                if(Gui.WantMouse || e.Button != MouseButton.Left)
                    return;
                MouseLook = true;
                mouselast = new Vector2(0, 0);
                window.CursorVisible = false;
            };
            window.MouseUp += (sender, e) => {
                if(Gui.WantMouse || e.Button != MouseButton.Left)
                    return;
                MouseLook = false;
                window.CursorVisible = true;
            };
        }

        public void AddPlaceable(Placeable placeable) {
            placeables.Add(placeable);
        }
        public void DeleteAll() {
            placeables = new List<Placeable>();
        }

        void Load() {
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

            Gui.Resize(window.Width, window.Height);
        }

        float[] frameTime = new float[240];
        float lastTime = -1;
        void Render() {
            GL.ClearColor(Color.MidnightBlue);
            GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);
            GL.Enable(EnableCap.DepthTest);
            GL.Enable(EnableCap.Blend);
            GL.BlendFunc(BlendingFactorSrc.SrcAlpha, BlendingFactorDest.OneMinusSrcAlpha);

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

            Gui.Render();

            window.SwapBuffers();

            for(var i = 0; i < frameTime.Length - 1; ++i)
                frameTime[i] = frameTime[i+1];
            var now = Time.Now;
            if(lastTime != -1)
                frameTime[frameTime.Length - 1] = now - lastTime;
            lastTime = now;
            var fps = 1 / (frameTime.Sum() / frameTime.Length);
            if(frameTime[0] != 0.0)
                window.Title = $"OpenEQ (FPS: {fps})";
        }

        Vector3 movementScale = new Vector3(4.0f, 4.0f, 1.0f);
        float runSpeed = 4, crawlSpeed = .1f;

        void Update() {
            while(GLTaskQueue.Count > 0)
                GLTaskQueue.Dequeue()();

            if(movement.Length != 0)
                camera.Translate(movement * movementScale);
            if(keylook.Length != 0)
                camera.Rotate(keylook / 50);
            if(MouseLook) {
                var mouse = Mouse.GetState();
                var mousepos = new Vector2(mouse.X, mouse.Y);
                var delta = (mousepos - mouselast) / 100;
                if(mouselast.X != 0 && mouselast.Y != 0 && (delta.X != 0 || delta.Y != 0))
                    camera.Rotate(delta);
                mouselast = mousepos;
            }
        }

        public void KeyDown(KeyboardKeyEventArgs e) {
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
                case Key.E:
                case Key.Space:
                    movement.Z += 1;
                    break;
                case Key.Q:
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
                case Key.ShiftLeft:
                    movementScale *= runSpeed;
                    break;
                case Key.AltLeft:
                    movementScale *= crawlSpeed;
                    break;
            }
        }
        public void KeyUp(KeyboardKeyEventArgs e) {
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
                case Key.E:
                case Key.Space:
                    movement.Z -= 1;
                    break;
                case Key.Q:
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
                case Key.ShiftLeft:
                    movementScale /= runSpeed;
                    break;
                case Key.AltLeft:
                    movementScale /= crawlSpeed;
                    break;
            }
        }

        public void Run() {
            window.Run(60, 60);
        }
    }
}