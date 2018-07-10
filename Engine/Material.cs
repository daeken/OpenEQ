﻿using System;
using System.IO;
using System.Linq;
using OpenTK.Graphics.OpenGL4;
using static OpenEQ.Engine.Globals;

namespace OpenEQ.Engine {
	[Flags]
	public enum MaterialFlag {
		Normal = 0, 
		Masked = 1, 
		Translucent = 2, 
		Transparent = 4
	}
	
	public class Material {
		readonly Texture[] Textures;
		readonly float AniParam;
		public readonly MaterialFlag Flags;
		
		public Material(MaterialFlag flags, float aniParam, Stream[] streams) {
			Flags = flags;
			AniParam = aniParam;
			Textures = streams.Select(stream => {
				using(var br = new BinaryReader(stream)) {
					var width = br.ReadInt32();
					var height = br.ReadInt32();
					var pixels = br.ReadBytes(width * height * 4);
					return new Texture((width, height), pixels, flags.HasFlag(MaterialFlag.Masked) && !flags.HasFlag(MaterialFlag.Translucent));
				}
			}).ToArray();
		}

		public void Use() {
			(Textures.Length == 1
				? Textures[0]
				: Textures[(int) MathF.Round(Stopwatch.ElapsedMilliseconds / AniParam) % Textures.Length]).Use();
		}
	}
}