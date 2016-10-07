using OpenTK;
using OpenTK.Graphics;
using OpenTK.Graphics.ES20;
using System;
using static System.Console;

namespace OpenEQ.Engine {
    public class CoreEngine {
        GameWindow window;

        Program program;
        Mesh mesh;
        Matrix4x4 model, view, perspective;

        public CoreEngine(Vec3[] verts, Vec3[] normals, Tuple<bool, int, int, int>[] polys) {
            verts = new Vec3[]{
                new Vec3(-1, -1,  1),
                new Vec3( 1, -1,  1),
                new Vec3( 1,  1,  1),
                new Vec3(-1,  1,  1),
                new Vec3(-1, -1, -1),
                new Vec3( 1, -1, -1),
                new Vec3( 1,  1, -1),
                new Vec3(-1,  1, -1),
            };
            normals = new Vec3[]{
                new Vec3(0, 0, 0), 
                new Vec3(0, 0, 1), 
                new Vec3(0, 1, 0), 
                new Vec3(0, 1, 1), 
                new Vec3(1, 0, 0), 
                new Vec3(1, 0, 1), 
                new Vec3(1, 1, 0), 
                new Vec3(1, 1, 1)
            };
            polys = new Tuple<bool, int, int, int>[]{
                new Tuple<bool, int, int, int>(false, 0, 1, 2), 
                new Tuple<bool, int, int, int>(false, 2, 3, 0), 

                new Tuple<bool, int, int, int>(false, 1, 5, 6),  
                new Tuple<bool, int, int, int>(false, 6, 2, 1), 

                new Tuple<bool, int, int, int>(false, 7, 6, 5), 
                new Tuple<bool, int, int, int>(false, 5, 4, 7),

                new Tuple<bool, int, int, int>(false, 4, 0, 3),
                new Tuple<bool, int, int, int>(false, 3, 7, 4),

                new Tuple<bool, int, int, int>(false, 4, 5, 1),
                new Tuple<bool, int, int, int>(false, 1, 0, 4),

                new Tuple<bool, int, int, int>(false, 3, 2, 6), 
                new Tuple<bool, int, int, int>(false, 6, 7, 3)
            };
            window = new GameWindow(1280, 720, GraphicsMode.Default, "OpenEQ");
            mesh = new Mesh(verts, normals, polys);

            model = Matrix4x4.Translation(new Vec3(0, -4, 0));
            view = Matrix4x4.LookAt(new Vec3(0, 0, 2), new Vec3(0, -4, 0), new Vec3(0, 0, 1));
            perspective = Matrix4x4.Perspective(45, ((float) window.Width) / window.Height, 1f, 10);
            
            window.Load += (sender, e) => Load();
            window.Resize += (sender, e) => Resize();
            window.RenderFrame += (sender, e) => Render();
        }

        void Load() {
            GL.ClearColor(Color.MidnightBlue);

            var vertshader = new Shader(@"
                    uniform mat4 MVPMatrix;
                    attribute vec4 vPosition;
                    attribute vec3 vNormal;
                    varying vec3 normal;
                    void main() {
                        gl_Position = MVPMatrix * vPosition;
                        normal = vNormal;
                    }
                ", ShaderType.VertexShader);
            
            var fragshader = new Shader(@"
                    varying vec3 normal;
                    void main() {
                        gl_FragColor = vec4(abs(normal), 1.0);
                    }
                ", ShaderType.FragmentShader);
            
            program = new Program(vertshader, fragshader);

            GL.CullFace(CullFaceMode.Back);
            //GL.Enable(EnableCap.CullFace);
        }

        void Resize() {
            WriteLine($"resizing {window.Width}x{window.Height}");
            GL.Viewport(0, 0, window.Width, window.Height);

            var depthbuffer = GL.GenRenderbuffer();
            GL.BindRenderbuffer(RenderbufferTarget.Renderbuffer, depthbuffer);
            GL.RenderbufferStorage(RenderbufferTarget.Renderbuffer, RenderbufferInternalFormat.DepthComponent16, window.Width, window.Height);
            GL.FramebufferRenderbuffer(FramebufferTarget.Framebuffer, All.DepthAttachment, RenderbufferTarget.Renderbuffer, depthbuffer);
            GL.Enable(EnableCap.DepthTest);
        }

        void Render() {
            GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);

            program.Use();
            //view *= Matrix4x4.Translation(new Vec3(0, .02f, 0));
            var mvp = perspective * view * model;
            program.Uniform("MVPMatrix", mvp.ToArray());
            mesh.Draw();

            window.SwapBuffers();
        }

        public void Run() {
            window.Run(60, 60);
        }
    }
}