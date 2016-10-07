using OpenTK.Graphics.ES20;
using static System.Console;
using System.Diagnostics;
using System.Collections.Generic;

namespace OpenEQ.Engine {
    public class Program {
        int id;
        Dictionary<string, int> uniforms;
        public Program(params Shader[] shaders) {
            id = GL.CreateProgram();
            foreach(var shader in shaders)
                GL.AttachShader(id, shader.ShaderId);
            
            GL.BindAttribLocation(id, 0, "vPosition");
            GL.LinkProgram(id);
            int linked;
            GL.GetProgram(id, GetProgramParameterName.LinkStatus, out linked);
            if(linked == 0) {
                WriteLine($"Program linking failed: {GL.GetProgramInfoLog(id)}");
                Debug.Assert(false);
            }

            uniforms = new Dictionary<string, int>();
        }

        public void Use() {
            GL.UseProgram(id);
            GL.EnableVertexAttribArray(GL.GetAttribLocation(id, "vPosition"));
            GL.EnableVertexAttribArray(GL.GetAttribLocation(id, "vNormal"));
        }

        public void Uniform(string name, float[] vals) {
            int uniform;
            if(uniforms.ContainsKey(name)) {
                uniform = uniforms[name];
            } else {
                uniform = uniforms[name] = GL.GetUniformLocation(id, name);
                WriteLine(GL.GetUniformLocation(id, name));
            }
            if(vals.Length == 16) {
                GL.UniformMatrix4(uniform, 1, true, vals);
            } else {
                WriteLine($"Unknown uniform length {vals.Length} for uniform {name}");
            }
        }
    }
}