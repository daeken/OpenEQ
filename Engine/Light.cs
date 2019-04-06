using System.Numerics;

namespace OpenEQ.Engine {
	public class PointLight {
		public Vector3 Color, Position;
		public float Radius, Attenuation;

		public PointLight(Vector3 position, float radius, float attenuation, Vector3 color) {
			Position = position;
			Radius = radius / 2;
			Attenuation = attenuation;
			Color = color;
		}
	}
}