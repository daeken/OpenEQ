using System.IO;

namespace OpenEQ {
    public class Vec3 {
        public float x, y, z;
        public Vec3() {
            x = y = z = 0.0f;
        }
        public Vec3(float _x, float _y, float _z) {
            x = _x;
            y = _y;
            z = _z;
        }

        public float dot(Vec3 b) {
            return x * b.x + y * b.y + z * b.z;
        }
        public static float operator %(Vec3 a, Vec3 b) {
            return a.dot(b);
        }

        public static Vec3 mix(Vec3 a, Vec3 b, float x) {
            return (b - a) * x + a;
        }

        public static Vec3 operator +(Vec3 a, Vec3 b) {
            return new Vec3(a.x + b.x, a.y + b.y, a.z + b.z);
        }
        public static Vec3 operator +(Vec3 a, float b) {
            return new Vec3(a.x + b, a.y + b, a.z + b);
        }
        public static Vec3 operator +(float a, Vec3 b) {
            return new Vec3(a + b.x, a + b.y, a + b.z);
        }
        public static Vec3 operator -(Vec3 a, Vec3 b) {
            return new Vec3(a.x - b.x, a.y - b.y, a.z - b.z);
        }
        public static Vec3 operator -(Vec3 a, float b) {
            return new Vec3(a.x - b, a.y - b, a.z - b);
        }
        public static Vec3 operator -(float a, Vec3 b) {
            return new Vec3(a - b.x, a - b.y, a - b.z);
        }
        public static Vec3 operator *(Vec3 a, Vec3 b) {
            return new Vec3(a.x * b.x, a.y * b.y, a.z * b.z);
        }
        public static Vec3 operator *(Vec3 a, float b) {
            return new Vec3(a.x * b, a.y * b, a.z * b);
        }
        public static Vec3 operator *(float a, Vec3 b) {
            return new Vec3(a * b.x, a * b.y, a * b.z);
        }
        public static Vec3 operator /(Vec3 a, Vec3 b) {
            return new Vec3(a.x / b.x, a.y / b.y, a.z / b.z);
        }
        public static Vec3 operator /(Vec3 a, float b) {
            return new Vec3(a.x / b, a.y / b, a.z / b);
        }
        public static Vec3 operator /(float a, Vec3 b) {
            return new Vec3(a / b.x, a / b.y, a / b.z);
        }

        public override string ToString() {
            return $"Vec3({x}, {y}, {z})";
        }
    }

    public static class Vec3Extensions {
        public static Vec3 ReadVec3(this BinaryReader reader) {
            return new Vec3(reader.ReadSingle(), reader.ReadSingle(), reader.ReadSingle());
        }
    }
}