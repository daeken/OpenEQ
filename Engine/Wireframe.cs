using System;
using System.Collections.Generic;
using System.Numerics;
using CollisionManager;
using OpenTK.Graphics.OpenGL4;

namespace OpenEQ.Engine {
	public class Wireframe {
		readonly Vao Vao;
		readonly Program Program;
		readonly int VertexCount;
		
		public Wireframe(IEnumerable<Triangle> mesh) {
			var vdata = new List<Vector3>();
			foreach(var tri in mesh) {
				vdata.Add(tri.A);
				vdata.Add(Vector3.UnitX);
				vdata.Add(tri.B);
				vdata.Add(Vector3.UnitY);
				vdata.Add(tri.C);
				vdata.Add(Vector3.UnitZ);
			}
			
			Vao = new Vao();
			Vao.Attach(new Buffer<Vector3>(vdata.ToArray()), (0, typeof(Vector3)), (1, typeof(Vector3)));

			VertexCount = vdata.Count / 6;
			
			Program = new Program(@"
#version 410
precision highp float;
layout (location = 0) in vec4 aPosition;
layout (location = 1) in vec3 aBarycentric;
uniform mat4 uProjectionViewMat;
out vec3 vBarycentric;

void main() {
	vBarycentric = aBarycentric;
	gl_Position = uProjectionViewMat * aPosition;
}
			", @"
#version 410
precision highp float;
in vec3 vBarycentric;
layout (location = 0) out vec4 color;
uniform sampler2D uTex;

vec3 fwidth(vec3 p) {
	return abs(dFdx(p)) + abs(dFdy(p));
}

void main() {
	vec3 d = fwidth(vBarycentric);
	vec3 a3 = smoothstep(vec3(0), d * 1.5, vBarycentric);
	color = vec4(1 - min(min(a3.x, a3.y), a3.z));
}
			");
		}
		
		public void Draw(Matrix4x4 projView) {
			Program.Use();
			Program.SetUniform("uProjectionViewMat", projView);
			GL.Disable(EnableCap.CullFace);
			GL.PolygonOffset(-10, -10);
			GL.Enable(EnableCap.PolygonOffsetFill);
			GL.Enable(EnableCap.Blend);
			Vao.Bind(() => GL.DrawArrays(PrimitiveType.Triangles, 0, VertexCount));
			GL.Enable(EnableCap.CullFace);
			GL.PolygonOffset(0, 0);
			GL.Disable(EnableCap.PolygonOffsetFill);
			GL.Disable(EnableCap.Blend);
		}
	}
}