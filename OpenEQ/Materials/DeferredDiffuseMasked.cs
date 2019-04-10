using System.Linq;
using System.Numerics;
using ImageLib;
using OpenEQ.Common;
using OpenEQ.Engine;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Materials {
	public class DeferredDiffuseMaskedMaterial : Material {
		public override bool Deferred => true;

		protected override string FragmentShader => @"
in vec2 vTexCoord;
in vec3 vNormal;
layout (location = 0) out vec4 color;
layout (location = 1) out vec3 normal;
uniform sampler2D uTex;
void main() {
	vec4 tcolor = texture(uTex, vTexCoord);
	if(tcolor.a < 0.5) discard;
	color = vec4(tcolor.rgb, 1.0);
	normal = vNormal;
	color = applyFog(color);
}
		";
		
		readonly Texture[] Textures;
		readonly float AnimationSpeed;
		
		public DeferredDiffuseMaskedMaterial(Image[] images, float animationSpeed = 0) {
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

		public override string ToString() => $"DeferredDiffuseMasked{Textures.Stringify()}";
	}
}