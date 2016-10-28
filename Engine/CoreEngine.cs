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

namespace OpenEQ.Engine {
    public class CoreEngine {
        public static Queue<Action> GLTaskQueue = new Queue<Action>();

        GameWindow window;

        Program zoneProgram, charProgram;
        List<Placeable> placeables;
        List<Mob> mobs;
        Matrix4 perspective;

        public Camera Camera;
        Vector3 movement;
        Vector2 mouselast, keylook;
        public bool MouseLook = false;

        public CoreEngine() {
            placeables = new List<Placeable>();
            mobs = new List<Mob>();
            window = new GameWindow(1280, 720, new GraphicsMode(new ColorFormat(32), 32), "OpenEQ", GameWindowFlags.Default, null, 4, 1, GraphicsContextFlags.ForwardCompatible);

            Camera = new Camera(new Vector3(0, 15, 0));
            perspective = Matrix4.CreatePerspectiveFieldOfView((float) (60 * Math.PI / 180), ((float) window.Width) / window.Height, 1, 10000);

            movement = new Vector3();
            keylook = new Vector2();
            
            window.Load += (sender, e) => Load();
            window.Resize += (sender, e) => Resize();
            window.UpdateFrame += (sender, e) => Update();
            window.RenderFrame += (sender, e) => Render();
            window.MouseDown += (sender, e) => {
                if(e.Button != MouseButton.Right)
                    return;
                MouseLook = true;
                mouselast = new Vector2(0, 0);
                window.CursorVisible = false;
            };
            window.MouseUp += (sender, e) => {
                if(e.Button != MouseButton.Right)
                    return;
                MouseLook = false;
                window.CursorVisible = true;
            };
        }

        public void AddPlaceable(Placeable placeable) {
            placeables.Add(placeable);
        }
        public void AddMob(Mob mob) {
            mobs.Add(mob);
        }
        public void DeleteAll() {
            placeables = new List<Placeable>();
            //mobs = new List<Mob>();
        }

        void Load() {
            zoneProgram = new Program(
                new Shader(File.ReadAllText("shaders/basevert.glsl"), ShaderType.VertexShader), 
                new Shader(File.ReadAllText("shaders/basefrag.glsl"), ShaderType.FragmentShader)
            );
            charProgram = new Program(
                new Shader(File.ReadAllText("shaders/charvert.glsl"), ShaderType.VertexShader),
                new Shader(File.ReadAllText("shaders/charfrag.glsl"), ShaderType.FragmentShader)
            );

            GL.CullFace(CullFaceMode.Back);
            GL.FrontFace(FrontFaceDirection.Cw);
            GL.Enable(EnableCap.CullFace);
        }

        void Resize() {
            WriteLine($"resizing {window.Width}x{window.Height}");
            GL.Viewport(0, 0, window.Width, window.Height);
        }

        float[] frameTime = new float[240];
        float lastTime = -1;
        void Render() {
            GL.ClearColor(Color.MidnightBlue);
            GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);
            GL.Enable(EnableCap.DepthTest);
            GL.Enable(EnableCap.Blend);
            GL.BlendFunc(BlendingFactorSrc.SrcAlpha, BlendingFactorDest.OneMinusSrcAlpha);

            zoneProgram.Use();
            var vp = Camera.Matrix * perspective;
            foreach(var placeable in placeables) {
                var model = placeable.Mat;
                var mvp = model * vp;
                var mv = model * Camera.Matrix;
                zoneProgram.Uniform("MVPMatrix", mvp);
                zoneProgram.Uniform("MVMatrix", mv);
                placeable.Draw();
            }

            charProgram.Use();
            foreach(var mob in mobs) {
                var model = Matrix4.Identity;
                var mvp = model * vp;
                var mv = model * Camera.Matrix;
                charProgram.Uniform("MVPMatrix", mvp);
                charProgram.Uniform("MVMatrix", mv);
                mob.Draw(charProgram);
            }

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
                Camera.Translate(movement * movementScale);
            if(keylook.Length != 0)
                Camera.Rotate(keylook / 50);
            if(MouseLook) {
                var mouse = Mouse.GetState();
                var mousepos = new Vector2(mouse.X, mouse.Y);
                var delta = (mousepos - mouselast) / 100;
                if(mouselast.X != 0 && mouselast.Y != 0 && (delta.X != 0 || delta.Y != 0))
                    Camera.Rotate(delta);
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