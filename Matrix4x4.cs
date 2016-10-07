using static System.Math;

namespace OpenEQ {
    public class Matrix4x4 {
        float a, b, c, d,
              e, f, g, h, 
              i, j, k, l, 
              m, n, o, p;
        float[] arr = null;
        public Matrix4x4() {
            a = f = k = p = 1; // Identity matrix
        }
        public Matrix4x4(
            float _a, float _b, float _c, float _d, 
            float _e, float _f, float _g, float _h, 
            float _i, float _j, float _k, float _l, 
            float _m, float _n, float _o, float _p
        ) {
            a = _a; b = _b; c = _c; d = _d;
            e = _e; f = _f; g = _g; h = _h;
            i = _i; j = _j; k = _k; l = _l;
            m = _m; n = _n; o = _o; p = _p;
        }

        public static Matrix4x4 Translation(Vec3 vec) {
            var mat = new Matrix4x4();
            mat.d = vec.x;
            mat.h = vec.y;
            mat.l = vec.z;
            return mat;
        }

        public static Matrix4x4 Scaling(Vec3 vec) {
            var mat = new Matrix4x4();
            mat.a = vec.x;
            mat.f = vec.y;
            mat.k = vec.z;
            return mat;
        }

        public static Matrix4x4 Perspective(float fov, float aspect, float near, float far) {
            var ymax = near * (float) Tan(fov * PI / 360);
            var xmax = ymax * aspect;
            return Frustumf2(-xmax, xmax, -ymax, ymax, near, far);
        }

        public static Matrix4x4 Frustumf2(float left, float right, float bottom, float top, float near, float far) {
            var temp = 2.0f * near;
            var temp2 = right - left;
            var temp3 = top - bottom;
            var temp4 = far - near;

            return new Matrix4x4(
                temp / temp2, 0, 0, 0, 
                0, temp / temp3, 0, 0, 
                (right + left) / temp2, (top + bottom) / temp3, (-far - near) / temp4, -1, 
                0, 0, (-temp * far) / temp4, 0
            );
        }

        static Vec3 ComputeNormalOfPlane(Vec3 b, Vec3 c) {
            return new Vec3(
                b.y*c.z - c.y*b.z, 
                b.z*c.x - c.z*b.x, 
                b.x*c.y - c.x*b.y
            );
        }

        public static Matrix4x4 LookAt(Vec3 eyePosition, Vec3 targetPosition, Vec3 up) {
            var forward = (targetPosition - eyePosition).normalize();
            var side = ComputeNormalOfPlane(forward, up).normalize();
            up = ComputeNormalOfPlane(side, forward);
            forward = -forward;

            return new Matrix4x4(
                side.x, up.x, forward.x, 0, 
                side.y, up.y, forward.y, 0, 
                side.z, up.z, forward.z, 0, 
                0, 0, 0, 0
            ) * Translation(-eyePosition);
        }

        public static Vec3 operator *(Matrix4x4 mat, Vec3 vec) {
            return new Vec3(
                mat.a * vec.x + mat.b * vec.y + mat.c * vec.z + mat.d, 
                mat.e * vec.x + mat.f * vec.y + mat.g * vec.z + mat.h,
                mat.i * vec.x + mat.j * vec.y + mat.k * vec.z + mat.l
            );
        }

        public static Matrix4x4 operator *(Matrix4x4 x, Matrix4x4 y) {
            return new Matrix4x4(
                x.a * y.a + x.b * y.e + x.c * y.i + x.d * y.m,
                x.a * y.b + x.b * y.f + x.c * y.j + x.d * y.n,
                x.a * y.c + x.b * y.g + x.c * y.k + x.d * y.o,
                x.a * y.d + x.b * y.h + x.c * y.l + x.d * y.p,

                x.e * y.a + x.f * y.e + x.g * y.i + x.h * y.m,
                x.e * y.b + x.f * y.f + x.g * y.j + x.h * y.n,
                x.e * y.c + x.f * y.g + x.g * y.k + x.h * y.o,
                x.e * y.d + x.f * y.h + x.g * y.l + x.h * y.p,

                x.i * y.a + x.j * y.e + x.k * y.i + x.l * y.m,
                x.i * y.b + x.j * y.f + x.k * y.j + x.l * y.n,
                x.i * y.c + x.j * y.g + x.k * y.k + x.l * y.o,
                x.i * y.d + x.j * y.h + x.k * y.l + x.l * y.p,

                x.m * y.a + x.n * y.e + x.o * y.i + x.p * y.m,
                x.m * y.b + x.n * y.f + x.o * y.j + x.p * y.n,
                x.m * y.c + x.n * y.g + x.o * y.k + x.p * y.o,
                x.m * y.d + x.n * y.h + x.o * y.l + x.p * y.p
            );
        }

        public float[] ToArray() {
            if(arr == null)
                arr = new float[]{a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p};
            return arr;
        }

        public override string ToString() {
            return $"Matrix4x4(\n\t{a}, {b}, {c}, {d}, \n\t{e}, {f}, {g}, {h}, \n\t{i}, {j}, {k}, {l}, \n\t{m}, {n}, {o}, {p}\n)";
        }
    }
}