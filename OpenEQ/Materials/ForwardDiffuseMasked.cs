using System.Linq;
using System.Numerics;
using ImageLib;
using OpenEQ.Common;
using OpenEQ.Engine;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Materials {
	public class ForwardDiffuseMaskedMaterial : Material {
		public override bool Deferred => false;

		protected override string FragmentShader => @"
in vec2 vTexCoord;
out vec4 color;
uniform sampler2D uTex;
void main() {
	color = texture(uTex, vTexCoord);
	color.a *= dot(color.rgb, vec3(1)) / 3;
	color = applyFog(color);
}
		";
		
		readonly Texture[] Textures;
		readonly float AnimationSpeed;
		
		public ForwardDiffuseMaskedMaterial(Image[] images, float animationSpeed = 0) {
			Textures = images.Select(image => new Texture(image, false)).ToArray();
			AnimationSpeed = animationSpeed;
		}

		protected override void UseInternal(Matrix4x4 projView, MaterialUse use) {
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

		public override string ToString() => $"ForwardDiffuseMasked{Textures.Stringify()}";
	}
}