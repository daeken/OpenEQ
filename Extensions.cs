using Godot;
using System.IO;

public static class Extensions {
	public static Vector2 ReadVector2(this BinaryReader br) {
		return new Vector2(br.ReadSingle(), br.ReadSingle());
	}
	public static Vector3 ReadVector3(this BinaryReader br) {
		return new Vector3(br.ReadSingle(), br.ReadSingle(), br.ReadSingle());
	}

	public static Quat EulerToQuat(this Vector3 v) {
		float c1 = Mathf.cos(v.x / 2), c2 = Mathf.cos(v.y / 2), c3 = Mathf.cos(v.z / 2);
		float s1 = Mathf.sin(v.x / 2), s2 = Mathf.sin(v.y / 2), s3 = Mathf.sin(v.z / 2);
		return new Quat(
			s1 * c2 * c3 + c1 * s2 * s3, 
			c1 * s2 * c3 - s1 * c2 * s3, 
			c1 * c2 * s3 + s1 * s2 * c3, 
			c1 * c2 * c3 - s1 * s2 * s3
		);
	}
}