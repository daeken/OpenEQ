using OpenTK.Graphics.OpenGL4;
using static System.Console;

namespace OpenEQ.Engine {
    public class Mesh {
        Material material;
        int vao, vertexBuffer, indexBuffer, indexCount;
        public Mesh(Material mat, float[] vertices, ushort[] indices) {
            material = mat;

            vao = GL.GenVertexArray();
            GL.BindVertexArray(vao);

            vertexBuffer = GL.GenBuffer();
            GL.BindBuffer(BufferTarget.ArrayBuffer, vertexBuffer);
            GL.BufferData(BufferTarget.ArrayBuffer, vertices.Length * sizeof(float), vertices, BufferUsageHint.StaticDraw);

            GL.EnableVertexAttribArray(0);
            GL.EnableVertexAttribArray(1);
            GL.EnableVertexAttribArray(2);
            GL.VertexAttribPointer(0, 3, VertexAttribPointerType.Float, false, 8 * sizeof(float), 0);
            GL.VertexAttribPointer(1, 3, VertexAttribPointerType.Float, false, 8 * sizeof(float), 3 * sizeof(float));
            GL.VertexAttribPointer(2, 2, VertexAttribPointerType.Float, false, 8 * sizeof(float), 6 * sizeof(float));

            indexBuffer = GL.GenBuffer();
            GL.BindBuffer(BufferTarget.ElementArrayBuffer, indexBuffer);
            GL.BufferData(BufferTarget.ElementArrayBuffer, indices.Length * sizeof(ushort), indices, BufferUsageHint.StaticDraw);
            indexCount = indices.Length;
        }

        public void Draw() {
            material.Enable();
            GL.BindVertexArray(vao);
            GL.DrawElements(BeginMode.Triangles, indexCount, DrawElementsType.UnsignedShort, 0);
        }
    }
}