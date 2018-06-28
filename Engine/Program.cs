using System;
using System.Collections.Generic;
using System.Linq;
using MoreLinq;
using OpenTK.Graphics.OpenGL4;
using static System.Console;

namespace OpenEQ.Engine {
	public class Program {
		static readonly Dictionary<string, int> ShaderCache = new Dictionary<string, int>();
		int ProgramId;
		readonly Dictionary<string, int> Locations = new Dictionary<string, int>();

		public Program(string vs, string fs) {
			ProgramId = GL.CreateProgram();
			GL.AttachShader(ProgramId, CompileShader(vs, ShaderType.VertexShader));
			GL.AttachShader(ProgramId, CompileShader(fs, ShaderType.FragmentShader));
			GL.LinkProgram(ProgramId);
			
			GL.GetProgram(ProgramId, GetProgramParameterName.LinkStatus, out var status);
			if(status != 1) {
				WriteLine($"Program linking failed: {GL.GetProgramInfoLog(ProgramId)}");
				throw new Exception("Shader linking failed");
			}
		}
		
		int CompileShader(string source, ShaderType type) {
			if(ShaderCache.ContainsKey(source))
				return ShaderCache[source];
			var shader = GL.CreateShader(type);
			GL.ShaderSource(shader, source);
			GL.CompileShader(shader);
			GL.GetShader(shader, ShaderParameter.CompileStatus, out var status);
			if(status != 1) {
				WriteLine($"Shader {type} compilation failed: {GL.GetShaderInfoLog(shader)}");
				throw new Exception("Shader compilation failed");
			}

			ShaderCache[source] = shader;
			return shader;
		}

		public void Use() => GL.UseProgram(ProgramId);

		public int GetUniform(string name) => Locations.ContainsKey(name)
			? Locations[name]
			: Locations[name] = GL.GetUniformLocation(ProgramId, name);
		
		public int GetAttribute(string name) => Locations.ContainsKey(name)
			? Locations[name]
			: Locations[name] = GL.GetAttribLocation(ProgramId, name);

		public void SetUniform(string name, int val) => GL.Uniform1(GetUniform(name), val);
		public void SetUniform(string name, double val) => GL.Uniform1(GetUniform(name), (float) val);
		public void SetUniform(string name, Vec3 val) => GL.Uniform3(GetUniform(name), 1, new[] { (float) val.X, (float) val.Y, (float) val.Z });
		public void SetUniform(string name, Mat4 val) => GL.UniformMatrix4(GetUniform(name), 1, false, val.AsArray.Select(x => (float) x).ToArray());

		public void SetTexture(string name, int tu, int tex) {
			GL.ActiveTexture(TextureUnit.Texture0 + tu);
			GL.BindTexture(TextureTarget.Texture2D, tex);
			SetUniform(name, tu);
		}

		public void SetTextures(int tu, int[] texs, params string[] names) {
			names.ForEach((name, i) => {
				SetTexture(name, tu + i, texs[i]);
			});
		}
	}
}