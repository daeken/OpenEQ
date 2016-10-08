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
        Matrix4 model, view, perspective;

        public CoreEngine(Vec3[] verts, Vec3[] normals, Tuple<float, float>[] texcoords, Tuple<bool, int, int, int>[] polys) {
            window = new GameWindow(1280, 720, GraphicsMode.Default, "OpenEQ");
            mesh = new Mesh(verts, normals, texcoords, polys);

            model = Matrix4.CreateTranslation(0, -1000, -1000);
            view = Matrix4.LookAt(new Vector3(0, 100, 150), new Vector3(0, 0, 0), new Vector3(0, 0, 1));
            perspective = Matrix4.CreatePerspectiveFieldOfView((float) (75 * Math.PI / 180), ((float) window.Width) / window.Height, .1f, 100000);
            
            window.Load += (sender, e) => Load();
            window.Resize += (sender, e) => Resize();
            window.RenderFrame += (sender, e) => Render();
        }

        void Load() {
            GL.ClearColor(Color.MidnightBlue);
            
            var vertshader = new Shader(@"
                    uniform mat4 MVPMatrix;
                    attribute vec3 vPosition;
                    attribute vec3 vNormal;
                    attribute vec2 vTexCoord;
                    varying vec3 normal;
                    varying vec2 texcoord;
                    void main() {
                        gl_Position = MVPMatrix * vec4(vPosition, 1.0);
                        normal = vNormal;
                        texcoord = vTexCoord;
                    }
                ", ShaderType.VertexShader);
            
            var fragshader = new Shader(@"
                    varying vec3 normal;
                    varying vec2 texcoord;
                    void main() {
                        gl_FragColor = vec4(abs(texcoord / 256.), 0.0, 1.0);
                    }
                ", ShaderType.FragmentShader);
            
            program = new Program(vertshader, fragshader);

            GL.CullFace(CullFaceMode.Back);
            GL.FrontFace(FrontFaceDirection.Cw);
            GL.Enable(EnableCap.CullFace);
        }

        void Resize() {
            WriteLine($"resizing {window.Width}x{window.Height}");
            GL.Viewport(0, 0, window.Width, window.Height);

            var depthbuffer = GL.GenRenderbuffer();
            GL.BindRenderbuffer(RenderbufferTarget.Renderbuffer, depthbuffer);
            GL.RenderbufferStorage(RenderbufferTarget.Renderbuffer, RenderbufferInternalFormat.DepthComponent16, window.Width, window.Height);
            GL.FramebufferRenderbuffer(FramebufferTarget.Framebuffer, All.DepthAttachment, RenderbufferTarget.Renderbuffer, depthbuffer);
            //GL.DepthFunc(DepthFunction.Lequal);
            //GL.DepthMask(false); // Makes a wireframe stippled object.  Odd.
        }

        void Render() {
            GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);
            GL.Enable(EnableCap.DepthTest);

            program.Use();
            //model *= Matrix4.CreateTranslation(0, -5, 0);
            var mvp = model * (view * perspective);
            program.Uniform("MVPMatrix", mvp);
            mesh.Draw();

            window.SwapBuffers();
        }

        public void Run() {
            window.Run(60, 60);
        }
    }
}