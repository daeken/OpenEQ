using System;
using System.IO;
using System.Linq;
using System.Numerics;
using ImageLib;
using OpenTK.Graphics.OpenGL4;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public enum MaterialUse {
		Static, 
		Animated
	}
	
	public abstract class Material {
		public abstract bool Deferred { get; }
		public virtual bool WantNormals => Deferred;
		
		protected abstract string FragmentShader { get; }

		Program StaticProgram, AnimatedProgram;

		string GenerateVertexShader(MaterialUse use) {
			switch(use) {
				case MaterialUse.Static: {
					var ret = @"
#version 410
precision highp float;
layout (location = 0) in vec4 aPosition;
layout (location = 1) in vec3 aNormal;
layout (location = 2) in vec2 aTexCoord;
layout (location = 3) in mat4 aModelMat;
uniform mat4 uProjectionViewMat;
out vec2 vTexCoord;
					";
					if(WantNormals)
						ret += @"
out vec3 vNormal;
						";
					ret += @"
void main() {
	gl_Position = uProjectionViewMat * aModelMat * aPosition;
	vTexCoord = aTexCoord;
					";
					if(WantNormals)
						ret += @"
	mat3 nmat = transpose(inverse(mat3(aModelMat)));
	vNormal = normalize(nmat * aNormal);
						";
					ret += @"
}
					";
					return ret;
				}
				case MaterialUse.Animated: {
					var ret = @"
#version 410
precision highp float;
layout (location = 0) in vec4 aPosition1;
layout (location = 1) in vec3 aNormal1;
layout (location = 2) in vec2 aTexCoord1;
layout (location = 3) in vec4 aPosition2;
layout (location = 4) in vec3 aNormal2;
layout (location = 5) in vec2 aTexCoord2;
uniform mat4 uModelMat;
uniform mat4 uProjectionViewMat;
uniform float uInterp;
out vec2 vTexCoord;
					";
					if(WantNormals)
						ret += @"
out vec3 vNormal;
						";
					ret += @"
void main() {
	gl_Position = uProjectionViewMat * uModelMat * mix(aPosition1, aPosition2, uInterp);
	vTexCoord = mix(aTexCoord1, aTexCoord2, uInterp);
					";
					if(WantNormals)
						ret += @"
	mat3 nmat = transpose(inverse(mat3(uModelMat)));
	vNormal = normalize(nmat * mix(aNormal1, aNormal2, uInterp));
						";
					ret += @"
}
					";
					return ret;
				}
			}
			return null;
		}
		
		protected Program GetProgram(MaterialUse use) {
			switch(use) {
				case MaterialUse.Static:
					return StaticProgram ?? (StaticProgram =
						       new Program(GenerateVertexShader(MaterialUse.Static), FragmentShader));
				case MaterialUse.Animated:
					return AnimatedProgram ?? (AnimatedProgram =
						       new Program(GenerateVertexShader(MaterialUse.Animated), FragmentShader));
			}
			return null;
		}
		
		public abstract void Use(Matrix4x4 projView, MaterialUse use);

		public void SetInterpolation(float interp) => AnimatedProgram.SetUniform("uInterp", interp);
	}
}