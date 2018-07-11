using System;
using OpenTK.Graphics.OpenGL4;
using OpenEQ.Common;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public class PointLight {
		public Vec3 Color, Position;
		public float Radius, Attenuation;

		public PointLight(Vec3 position, float radius, float attenuation, Vec3 color) {
			Position = position;
			Radius = radius / 3;
			Attenuation = attenuation;
			Color = color;
		}
	}
}