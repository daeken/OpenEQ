using System;
using OpenTK.Graphics.OpenGL4;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class PointLight {
		public static Program FinalProgram;
		
		public Vec3 Color;

		public Vec3 Position;
		public float Radius, Attenuation;

		public PointLight(Vec3 position, float radius, float attenuation, Vec3 color) {
			Position = position;
			Radius = radius / 2;
			Attenuation = attenuation;
			Color = color;
			
			if(FinalProgram == null)
				FinalProgram = new Program(@"
#version 410
precision highp float;
in vec2 aPosition;
out vec2 vTexCoord;
uniform mat4 uProjectionViewMat, uModelMat;
void main() {
	gl_Position = vec4(aPosition, 0.0, 1.0);
	vTexCoord = aPosition.xy * 0.5 + 0.5;
}
				", @"
#version 410
precision highp float;
in vec2 vTexCoord;

uniform sampler2D uColor, uPosition, uNormal;
uniform vec3 uLightPos, uLightColor, uCameraPos;
uniform float uRadius;//, uAttenuation;
out vec3 color;

void main() {
	vec3 pos = texture(uPosition, vTexCoord).xyz;
	vec3 toLight = uLightPos - pos;
	float dist = length(toLight);
	//if(dist > uRadius) discard;
	//vec3 N = texture(uNormal, vTexCoord).xyz;
	//if(length(N) < 0.9) discard;
	//toLight = normalize(toLight);
	//float cos = clamp(dot(N, toLight), 0, 1);
	//if(cos == 0) discard;
	vec4 csv = texture(uColor, vTexCoord);
	float diffuse = pow(1 - min(dist / uRadius, 1), 3);// * cos;
	color = csv.rgb * uLightColor * diffuse;
}
				");
		}
		
		public static void SetupFinal(int[] textures, Vec3 cameraPos) {
			FinalProgram.Use();
			FinalProgram.SetUniform("uCameraPos", cameraPos);
			FinalProgram.SetTextures(0, textures, "uColor", "uPosition", "uNormal");
		}

		public void SetupIndividual() {
			FinalProgram.Use();
			FinalProgram.SetUniform("uLightColor", Color);
			FinalProgram.SetUniform("uLightPos", Position);
			FinalProgram.SetUniform("uRadius", Radius);
			//FinalProgram.SetUniform("uAttenuation", Attenuation);
		}
	}
}