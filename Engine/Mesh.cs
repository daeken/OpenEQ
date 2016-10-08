using OpenTK.Graphics.ES20;
using System;
using static System.Console;
using System.Collections.Generic;
using System.Linq;
using System.Diagnostics;
using System.IO;

namespace OpenEQ.Engine {
    public class Mesh {
        List<Tuple<int, int, int>> buffers; // VBuffer * IBuffer * numinds
        public Mesh(Vec3[] verts, Vec3[] normals, Tuple<float, float>[] texcoords, Tuple<bool, int, int, int>[] polys) {
            /*using(var f = new StreamWriter(File.Open("zone.stl", FileMode.Create))) {
                WriteLine("building stl");
                f.Write("solid zone\n");
                foreach(var poly in polys) {
                    f.Write("facet normal 0 0 0\n\touter loop\n");
                    f.Write($"\t\tvertex {verts[poly.Item2].x:f} {verts[poly.Item2].y:f} {verts[poly.Item2].z:f}\n");
                    f.Write($"\t\tvertex {verts[poly.Item3].x:f} {verts[poly.Item3].y:f} {verts[poly.Item3].z:f}\n");
                    f.Write($"\t\tvertex {verts[poly.Item4].x:f} {verts[poly.Item4].y:f} {verts[poly.Item4].z:f}\n");
                    f.Write("\tendloop\nendfacet\n");
                }
                f.Write("endsolid zone\n");
            }*/

            Debug.Assert(verts.Length == normals.Length);
            Debug.Assert(verts.Length == texcoords.Length);
            buffers = new List<Tuple<int, int, int>>(); // vertex buffer * index buffer * numinds

            if(verts.Length > 65536) {
                var cinds = new List<int>();
                var cverts = new HashSet<int>();
                foreach(var poly in polys) {
                    var na = !cverts.Contains(poly.Item2);
                    var nb = !cverts.Contains(poly.Item3);
                    var nc = !cverts.Contains(poly.Item4);
                    var ncount = (na ? 1 : 0) + (nb ? 1 : 0) + (nc ? 1 : 0);
                    if(cverts.Count + ncount > 65536) {
                        BuildBuffer(cinds, verts, normals, texcoords, cverts.Count);
                        cinds = new List<int>();
                        cverts = new HashSet<int>();
                        na = nb = nc = true;
                    }
                    if(na) cverts.Add(poly.Item2);
                    if(nb) cverts.Add(poly.Item3);
                    if(nc) cverts.Add(poly.Item4);
                    cinds.Add(poly.Item2);
                    cinds.Add(poly.Item3);
                    cinds.Add(poly.Item4);
                }

                if(cinds.Count > 0)
                    BuildBuffer(cinds, verts, normals, texcoords, cverts.Count);
            } else {
                var cinds = new List<int>();
                foreach(var poly in polys) {
                    cinds.Add(poly.Item2);
                    cinds.Add(poly.Item3);
                    cinds.Add(poly.Item4);
                }
                BuildBuffer(cinds, verts, normals, texcoords, verts.Length);
            }
        }

        void BuildBuffer(List<int> cinds, Vec3[] verts, Vec3[] normals, Tuple<float, float>[] texcoords, int vcount) {
            var nverts = new float[vcount * 8];
            ushort[] ninds;
            if(verts.Length > 65536) {
                ninds = new ushort[cinds.Count];
                var used = new Dictionary<int, ushort>();
                
                for(var i = 0; i < cinds.Count; ++i) {
                    var v = cinds[i];
                    if(!used.ContainsKey(v)) {
                        ninds[i] = (ushort) used.Count;
                        var bv = used.Count * 8;
                        nverts[bv + 0] = verts[v].x;
                        nverts[bv + 1] = verts[v].y;
                        nverts[bv + 2] = verts[v].z;
                        nverts[bv + 3] = normals[v].x;
                        nverts[bv + 4] = normals[v].y;
                        nverts[bv + 5] = normals[v].z;
                        nverts[bv + 6] = texcoords[v].Item1;
                        nverts[bv + 7] = texcoords[v].Item2;
                        used[v] = ninds[i];
                    } else {
                        ninds[i] = used[v];
                    }
                }
            } else {
                ninds = cinds.Select(v => (ushort) v).ToArray();
                for(var i = 0; i < verts.Length; ++i) {
                    nverts[i * 8 + 0] = verts[i].x;
                    nverts[i * 8 + 1] = verts[i].y;
                    nverts[i * 8 + 2] = verts[i].z;
                    nverts[i * 8 + 3] = normals[i].x;
                    nverts[i * 8 + 4] = normals[i].y;
                    nverts[i * 8 + 5] = normals[i].z;
                    nverts[i * 8 + 6] = texcoords[i].Item1;
                    nverts[i * 8 + 7] = texcoords[i].Item2;
                }
            }

            var vbuffer = GL.GenBuffer();
            GL.BindBuffer(BufferTarget.ArrayBuffer, vbuffer);
            GL.BufferData(BufferTarget.ArrayBuffer, nverts.Length * sizeof(float), nverts, BufferUsageHint.StaticDraw);
            var ibuffer = GL.GenBuffer();
            GL.BindBuffer(BufferTarget.ElementArrayBuffer, ibuffer);
            GL.BufferData(BufferTarget.ElementArrayBuffer, ninds.Length * sizeof(ushort), ninds, BufferUsageHint.StaticDraw);

            buffers.Add(new Tuple<int, int, int>(vbuffer, ibuffer, ninds.Length));
        }

        public void Draw() {
            foreach(var cbuf in buffers) {
                WriteLine($"Drawing buffer of {cbuf.Item3} indices ({cbuf.Item1} - {cbuf.Item2})");
                GL.BindBuffer(BufferTarget.ArrayBuffer, cbuf.Item1);
                GL.BindBuffer(BufferTarget.ElementArrayBuffer, cbuf.Item2);
                GL.VertexAttribPointer(0, 3, VertexAttribPointerType.Float, false, 8 * sizeof(float), 0);
                GL.VertexAttribPointer(1, 3, VertexAttribPointerType.Float, false, 8 * sizeof(float), 3 * sizeof(float));
                GL.VertexAttribPointer(2, 3, VertexAttribPointerType.Float, false, 8 * sizeof(float), 6 * sizeof(float));
                GL.DrawElements(BeginMode.Triangles, cbuf.Item3, DrawElementsType.UnsignedShort, 0);
            }
        }
    }
}