using System.Linq;
using System.Numerics;
using ImageLib;
using OpenEQ.Engine;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Materials {
	public class ForwardDiffuseMaterial : Material {
		public override bool Deferred => false;

		protected override string FragmentShader => @"
#version 410
precision highp float;
in vec3 vPosition;
in vec2 vTexCoord;
out vec4 color;
uniform sampler2D uTex;
void main() {
	color = texture(uTex, vTexCoord);
}
		";
		
		readonly Texture[] Textures;
		readonly float AnimationSpeed;
		
		public ForwardDiffuseMaterial(Image[] images, float animationSpeed = 0) {
			Textures = images.Select(image => new Texture(image, false)).ToArray();
			AnimationSpeed = animationSpeed;
		}

		public override void Use(Matrix4x4 projView, MaterialUse use) {
			var program = GetProgram(use);
			program.Use();
			program.SetUniform("uTex", 0);
			program.SetUniform("uProjectionViewMat", projView);
			GL.ActiveTexture(TextureUnit.Texture0);
			if(AnimationSpeed == 0)
				Textures[0].Use();
			else
				Textures[(int) (Globals.Time / AnimationSpeed) % Textures.Length].Use();
		}
	}
}