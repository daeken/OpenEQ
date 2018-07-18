using System;
using System.IO;
using System.Linq;
using System.Numerics;
using ImageLib;
using OpenTK.Graphics.OpenGL4;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	public abstract class Material {
		public abstract bool Deferred { get; }
		
		public abstract void Use(Matrix4x4 projView);
	}
}