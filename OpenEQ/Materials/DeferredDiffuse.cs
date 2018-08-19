using System.Linq;
using System.Numerics;
using ImageLib;
using OpenEQ.Common;
using OpenEQ.Engine;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Materials {
	public class DeferredDiffuseMaterial : Material {
		public override bool Deferred => true;

		protected override string FragmentShader => @"
#version 410
precision highp float;
in vec2 vTexCoord;
in vec3 vNormal;
layout (location = 0) out vec3 color;
layout (location = 1) out vec3 normal;
uniform sampler2D uTex;
void main() {
	color = texture(uTex, vTexCoord).rgb;
	normal = vNormal;
}
		";

		readonly Texture[] Textures;
		readonly float AnimationSpeed;
		
		public DeferredDiffuseMaterial(Image[] images, float animationSpeed = 0) {
			Textures = images.Select(image => new Texture(image, false)).ToArray();
			AnimationSpeed = animationSpeed;
		}

		public override void Use(Matrix4x4 projView, MaterialUse use) {
			var program = GetProgram(use);
			program.Use();
			program.SetUniform("uTex", 0);
			program.SetUniform("uProjectionViewMat", projView);
			program.SetUniform("uModelMat", Matrix4x4.Identity);
			GL.ActiveTexture(TextureUnit.Texture0);
			if(AnimationSpeed == 0)
				Textures[0].Use();
			else
				Textures[(int) (Globals.Time / AnimationSpeed) % Textures.Length].Use();
		}

		public override string ToString() => $"DeferredDiffuse{Textures.Stringify()}";
	}
}