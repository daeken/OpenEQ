using System;
using System.Collections.Generic;
using OpenTK.Graphics.OpenGL4;
using System.Linq;
using static System.Console;

namespace OpenEQ.Engine {
    public class CharacterModel {
        List<string> animationNames;
        
        public List<string> AnimationNames {
            get {
                return animationNames;
            }
        }
        string curAnimation = "";
        float animationStartTime;
        public string Animation {
            get { return curAnimation; }
            set {
                curAnimation = value;
                animationStartTime = Time.Now;
            }
        }

        Tuple<Material, int, int>[] indexBuffers; // Material * bufferId * length
        Dictionary<string, int[]> vertexBuffers = new Dictionary<string, int[]>(); // Array of bufferId

        public CharacterModel(Tuple<Material, uint[]>[] matpolys, Dictionary<string, float[][]> animations) {
            animationNames = animations.Keys.ToList();
            animationStartTime = Time.Now;

            foreach(var animation in animations) {
                var thisani = vertexBuffers[animation.Key] = new int[animation.Value.Length];
                for(var i = 0; i < animation.Value.Length; ++i) {
                    var vertices = animation.Value[i];
                    var vertexBuffer = thisani[i] = GL.GenBuffer();
                    GL.BindBuffer(BufferTarget.ArrayBuffer, vertexBuffer);
                    GL.BufferData(BufferTarget.ArrayBuffer, vertices.Length * sizeof(float), vertices, BufferUsageHint.StaticDraw);
                }
            }

            indexBuffers = new Tuple<Material, int, int>[matpolys.Length];
            for(var i = 0; i < matpolys.Length; ++i) {
                var indices = matpolys[i].Item2;
                var indexBuffer = GL.GenBuffer();
                GL.BindBuffer(BufferTarget.ElementArrayBuffer, indexBuffer);
                GL.BufferData(BufferTarget.ElementArrayBuffer, indices.Length * sizeof(uint), indices, BufferUsageHint.StaticDraw);
                indexBuffers[i] = new Tuple<Material, int, int>(matpolys[i].Item1, indexBuffer, indices.Length);
            }
        }

        public void Draw(Program program) {
            var ani = vertexBuffers[curAnimation];
            var tdelta = Time.Now - animationStartTime;
            var framenum = (int)(tdelta * 10);
            var framepos = (tdelta - (framenum / 10f)) * 10f;
            GL.Uniform1(2, framepos);
            GL.BindBuffer(BufferTarget.ArrayBuffer, ani[framenum % ani.Length]);

            int vPos = program.GetAttributeLocation("vPosition"), vNewPos = program.GetAttributeLocation("vNewPosition"),
                vNorm = program.GetAttributeLocation("vNormal"), vNewNorm = program.GetAttributeLocation("vNewNormal"),
                vTex = program.GetAttributeLocation("vTexCoord");

            GL.EnableVertexAttribArray(vPos);
            GL.EnableVertexAttribArray(vNorm);
            GL.EnableVertexAttribArray(vTex);
            GL.EnableVertexAttribArray(vNewPos);
            GL.EnableVertexAttribArray(vNewNorm);
            GL.VertexAttribPointer(vPos, 3, VertexAttribPointerType.Float, false, 8 * sizeof(float), 0);
            GL.VertexAttribPointer(vNorm, 3, VertexAttribPointerType.Float, false, 8 * sizeof(float), 3 * sizeof(float));
            GL.VertexAttribPointer(vTex, 2, VertexAttribPointerType.Float, false, 8 * sizeof(float), 6 * sizeof(float));
            GL.BindBuffer(BufferTarget.ArrayBuffer, ani[(framenum + 1) % ani.Length]);
            GL.VertexAttribPointer(vNewPos, 3, VertexAttribPointerType.Float, false, 8 * sizeof(float), 0);
            GL.VertexAttribPointer(vNewNorm, 3, VertexAttribPointerType.Float, false, 8 * sizeof(float), 3 * sizeof(float));

            foreach(var elem in indexBuffers) {
                if(!elem.Item1.Enable())
                    continue;
                GL.BindBuffer(BufferTarget.ElementArrayBuffer, elem.Item2);
                GL.DrawElements(BeginMode.Triangles, elem.Item3, DrawElementsType.UnsignedInt, 0);
            }
        }
    }
}
