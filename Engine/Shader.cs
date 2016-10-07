using OpenTK;
using OpenTK.Graphics;
using OpenTK.Graphics.ES20;
using System;
using static System.Console;
using System.Diagnostics;

namespace OpenEQ.Engine {
    public class Shader {
        public int ShaderId;
        public Shader(string source, ShaderType type) {
            ShaderId = GL.CreateShader(type);
            Debug.Assert(ShaderId != 0);

            GL.ShaderSource(ShaderId, source);
            GL.CompileShader(ShaderId);

            int compiled;
            GL.GetShader(ShaderId, ShaderParameter.CompileStatus, out compiled);
            if(compiled == 0) {
                WriteLine($"Shader compilation failed: {GL.GetShaderInfoLog(ShaderId)}");
                Debug.Assert(false);
            }
        }
    }
}