using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using MoreLinq;
using OpenTK.Graphics.OpenGL4;
using OpenEQ.Common;
using static System.Console;

namespace OpenEQ.Engine {
	public class Program {
		static readonly Dictionary<(string, string), int> ProgramCache = new Dictionary<(string, string), int>();
		static readonly Dictionary<string, int> ShaderCache = new Dictionary<string, int>();
		readonly int Id;
		readonly Dictionary<string, int> Locations = new Dictionary<string, int>();
		public static Program Current;

		public Program(string vs, string fs) {
			var key = (vs, fs);
			if(ProgramCache.ContainsKey(key))
				Id = ProgramCache[key];
			else {
				Id = GL.CreateProgram();
				GL.AttachShader(Id, CompileShader(vs, ShaderType.VertexShader));
				GL.AttachShader(Id, CompileShader(fs, ShaderType.FragmentShader));
				GL.LinkProgram(Id);

				GL.GetProgram(Id, GetProgramParameterName.LinkStatus, out var status);
				if(status != 1) {
					WriteLine($"Program linking failed: {GL.GetProgramInfoLog(Id)}");
					throw new Exception("Shader linking failed");
				}
				
				ProgramCache[key] = Id;
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

		public void Use() {
			if(Current == this) return;
			GL.UseProgram(Id);
			Current = this;
		}

		public int GetUniform(string name) => Locations.ContainsKey(name)
			? Locations[name]
			: Locations[name] = GL.GetUniformLocation(Id, name);
		
		public int GetAttribute(string name) => Locations.ContainsKey(name)
			? Locations[name]
			: Locations[name] = GL.GetAttribLocation(Id, name);

		public void SetUniform(string name, int val) => GL.Uniform1(GetUniform(name), val);
		public void SetUniform(string name, float val) => GL.Uniform1(GetUniform(name), val);
		public void SetUniform(string name, Vector2 val) => GL.Uniform2(GetUniform(name), 1, ref val.X);
		public void SetUniform(string name, Vector3 val) => GL.Uniform3(GetUniform(name), 1, ref val.X);
		public void SetUniform(string name, Matrix4x4 val) => GL.UniformMatrix4(GetUniform(name), 1, false, ref val.M11);

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