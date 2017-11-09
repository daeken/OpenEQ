using Godot;
using OpenEQ.Network;
using System;
using System.IO;

public static class Extensions {
	public static Vector2 ReadVector2(this BinaryReader br) {
		return new Vector2(br.ReadSingle(), br.ReadSingle());
	}
	public static Vector3 ReadVector3(this BinaryReader br) {
		return new Vector3(br.ReadSingle(), br.ReadSingle(), br.ReadSingle());
	}
	public static Quat ReadQuat(this BinaryReader br) {
		return new Quat(br.ReadSingle(), br.ReadSingle(), br.ReadSingle(), br.ReadSingle());
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

	public static Transform ToTransform(this Tuple<Vector3, Quat> data) {
		return new Transform(data.Item2, data.Item1);
	}

	public static Tuple<float, float, float, float> GetPositionHeading(this SpawnPosition position) {
		return new Tuple<float, float, float, float>(
			position.Y / 8f,
			position.Z / 8f,
			position.X / 8f,
			position.Heading / 8f / 255f
		);
	}

	public static Tuple<float, float, float, float> GetPositionHeading(this UpdatePosition position) {
		return new Tuple<float, float, float, float>(
			position.Y / 8f,
			position.Z / 8f,
			position.X / 8f,
			position.Heading / 8f / 255f
		);
	}

	public static Tuple<float, float, float, float> GetDeltas(this UpdatePosition position) {
		return new Tuple<float, float, float, float>(
			position.DeltaY / 64f,
			position.DeltaZ / 64f,
			position.DeltaX / 64f,
			position.DeltaHeading / 64f / 255f
		);
	}

	public static Vector3 XYZ(this Tuple<float, float, float> data) {
		return new Vector3(data.Item1, data.Item2, data.Item3);
	}

	public static Vector3 XYZ(this Tuple<float, float, float, float> data) {
		return new Vector3(data.Item1, data.Item2, data.Item3);
	}
}