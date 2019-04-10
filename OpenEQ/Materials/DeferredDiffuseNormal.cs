﻿using System.Linq;
using System.Numerics;
using ImageLib;
using OpenEQ.Engine;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Materials {
	public class DeferredDiffuseNormalMaterial : Material {
		public override bool Deferred => true;

		protected override string FragmentShader => @"
		in vec2 vTexCoord;
		in vec3 vNormal;
		layout (location = 0) out vec4 color;
		layout (location = 1) out vec3 normal;
		uniform sampler2D uDiffuseTex, uNormalTex;
		void main() {
			color = vec4(texture(uDiffuseTex, vTexCoord).rgb, 1.0);
			normal = normalize(texture(uNormalTex, vTexCoord).xyz * 2 - 1 + vNormal);
			color = applyFog(color);
		}
		";

		readonly Texture[] Textures;
		
		public DeferredDiffuseNormalMaterial(Image[] images) =>
			Textures = images.Select(image => new Texture(image, false)).ToArray();

		protected override void UseInternal(Matrix4x4 projView, MaterialUse use) {
			var program = GetProgram(use);
			program.Use();
			program.SetUniform("uDiffuseTex", 0);
			program.SetUniform("uNormalTex", 1);
			program.SetUniform("uProjectionViewMat", projView);
			program.SetUniform("uModelMat", Matrix4x4.Identity);
			GL.ActiveTexture(TextureUnit.Texture0);
			Textures[0].Use();
			GL.ActiveTexture(TextureUnit.Texture1);
			Textures[1].Use();
		}
	}
}