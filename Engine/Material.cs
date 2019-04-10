using System;
using System.Numerics;

namespace OpenEQ.Engine {
	public enum MaterialUse {
		Static, 
		Animated
	}
	
	public abstract class Material {
		public abstract bool Deferred { get; }
		public virtual bool WantNormals => Deferred;
		
		protected abstract string FragmentShader { get; }

		string FullFragmentShader => @"
#version 410
precision highp float;
uniform vec3 uFogColor;
uniform float uFogDensity;
in vec4 vPosition;
vec4 applyFog(vec4 color) {
	float dist = log(vPosition.z);
	return vec4(mix(color.rgb, uFogColor * mix(0.75 + (color.r + color.g + color.b) / 12, 0.75, smoothstep(log(1000), log(1050), dist)), clamp(pow(max(0, dist - log(250)), 3), 0, 1)), color.a);
}
		" + FragmentShader;

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
uniform mat4 uModelMat;
uniform mat4 uProjectionViewMat;
out vec2 vTexCoord;
out vec4 vPosition;
					";
					if(WantNormals)
						ret += @"
out vec3 vNormal;
						";
					ret += @"
void main() {
	gl_Position = vPosition = uProjectionViewMat * uModelMat * aPosition;
	vTexCoord = aTexCoord;
					";
					if(WantNormals)
						ret += @"
	mat3 nmat = transpose(inverse(mat3(uModelMat)));
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
out vec4 vPosition;
					";
					if(WantNormals)
						ret += @"
out vec3 vNormal;
						";
					ret += @"
void main() {
	gl_Position = vPosition = uProjectionViewMat * uModelMat * mix(aPosition1, aPosition2, uInterp);
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

		protected Program GetProgram(MaterialUse use) => use switch {
			MaterialUse.Static =>
				StaticProgram ??= new Program(GenerateVertexShader(MaterialUse.Static), FullFragmentShader),
			MaterialUse.Animated =>
				AnimatedProgram ??= new Program(GenerateVertexShader(MaterialUse.Animated), FullFragmentShader), 
			_ => throw new NotImplementedException($"Unknown MaterialUse: {use}")
		};
		
		protected abstract void UseInternal(Matrix4x4 projView, MaterialUse use);

		public void Use(Matrix4x4 projView, MaterialUse use) {
			UseInternal(projView, use);
			var program = GetProgram(use);
			var fog = 0.6f;
			program.SetUniform("uFogColor", new Vector3(fog));
		}

		public void SetModelMatrix(Matrix4x4 modelMat) {
			StaticProgram?.SetUniform("uModelMat", modelMat);
			AnimatedProgram?.SetUniform("uModelMat", modelMat);
		}

		public void SetInterpolation(float interp) => AnimatedProgram.SetUniform("uInterp", interp);
	}
}