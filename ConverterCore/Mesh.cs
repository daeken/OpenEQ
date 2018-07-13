using System.Collections.Generic;
using System.Linq;
using MoreLinq;
using OpenEQ.Common;
using OpenEQ.LegacyFileReader;
using static System.Console;

namespace OpenEQ.ConverterCore {
	public class MeshPiece {
		public readonly List<Vec3> Vertices, Normals;
		public readonly List<Vec2> TexCoords;
		public readonly List<(bool, uint, uint, uint)> Polygons;
		public readonly List<(uint Flags, uint AnimSpeed, List<string> Filenames)> Textures;
		public readonly List<(uint Count, uint Index)> PolyTexs;

		public MeshPiece(Fragment36 meshfrag) {
			Vertices = meshfrag.Vertices.ToList();
			Normals = meshfrag.Normals.ToList();
			TexCoords = meshfrag.TexCoords.ToList();
			Polygons = meshfrag.Polygons.ToList();
			Textures = meshfrag.TextureListReference.Value.References.Select(x => {
				var sr1 = x.Value;
				if(sr1.Reference.Value == null) return (0, 0, null);
				var sr2 = sr1.Reference.Value;
				var sr = sr2.Reference.Value;
				return (x.Value.Flags, sr.FrameTime,
					sr.References.Select(y => y.Value.Filenames.ToList()).SelectMany(y => y).ToList());
			}).Where(x => x.Item3 != null).ToList();
			PolyTexs = meshfrag.PolyTexs.ToList();
		}
	}
	
	public class Mesh {
		readonly List<MeshPiece> Pieces = new List<MeshPiece>();

		public void Add(MeshPiece piece) => Pieces.Add(piece);

		public List<(float[] VertexBuffer, uint[] IndexBuffer, bool Collidable, (uint Flags, uint AnimSpeed, List<string> Filenames) Texture)>
			Bake()
		{
			var verts = new List<Vec3>();
			var normals = new List<Vec3>();
			var texCoords = new List<Vec2>();
			var textures = new List<(uint Flags, uint AnimSpeed, List<string> Filenames)>();
			var polygons = new Dictionary<(int TextureIndex, bool Collidable), List<(uint, uint, uint)>>();

			foreach(var piece in Pieces) {
				var vertoff = (uint) verts.Count;
				var texoff = textures.Count;
				verts.AddRange(piece.Vertices);
				normals.AddRange(piece.Normals.Concat(Enumerable.Range(0, piece.Vertices.Count - piece.Normals.Count).Select(x => Vec3.One)));
				texCoords.AddRange(piece.TexCoords.Concat(Enumerable.Range(0, piece.Vertices.Count - piece.TexCoords.Count).Select(x => Vec2.Zero)));
				textures.AddRange(piece.Textures);
				var pi = 0;
				foreach(var (ptc, ti) in piece.PolyTexs) {
					foreach(var (collidable, a, b, c) in piece.Polygons.Skip(pi).Take((int) ptc)) {
						var index = ((int) ti + texoff, collidable);
						if(!polygons.ContainsKey(index))
							polygons[index] = new List<(uint, uint, uint)>();
						polygons[index].Add((a + vertoff, b + vertoff, c + vertoff));
					}
					pi += (int) ptc;
				}
			}

			var optTextures = new List<(uint Flags, uint AnimSpeed, string Filenames)>();
			var texIndex = new Dictionary<(uint Flags, uint AnimSpeed, string Filenames), int>();
			var texMap = new Dictionary<int, int>();
			textures.ForEach((texture, i) => {
				var index = (texture.Flags, texture.AnimSpeed, string.Join(',', texture.Filenames));
				if(!texIndex.ContainsKey(index)) {
					texIndex[index] = optTextures.Count;
					optTextures.Add(index);
				}
				texMap[i] = texIndex[index];
			});
			var optPolygons = new Dictionary<(int TextureIndex, bool Collidable), List<(uint, uint, uint)>>();
			foreach(var ((ti, c), polys) in polygons) {
				var index = (texMap[ti], c);
				if(!optPolygons.ContainsKey(index))
					optPolygons[index] = polys;
				else
					optPolygons[index].AddRange(polys);
			}

			var meshes = new List<(float[], uint[], bool, (uint, uint, List<string>))>();
			foreach(var ((ti, c), polys) in optPolygons) {
				var (pvb, pib) = SplitPolyMesh(verts, normals, texCoords, polys);
				var (flags, ani, fns) = optTextures[ti];
				meshes.Add((pvb, pib, c, (flags, ani, fns.Split(',').ToList())));
			}
			return meshes;
		}

		(float[], uint[]) SplitPolyMesh(List<Vec3> vb, List<Vec3> nb, List<Vec2> tcb, List<(uint, uint, uint)> polys) {
			var ovb = new List<float>();
			var vmap = new Dictionary<(Vec3, Vec3, Vec2), uint>();
			var oib = new List<uint>();

			uint Add(uint i) {
				var (v, n, t) = (vb[(int) i], nb[(int) i], tcb[(int) i]);
				var key = (v, n, t);
				if(vmap.ContainsKey(key))
					return vmap[key];
				var ind = vmap[key] = (uint) (ovb.Count / 8);
				ovb.AddRange(v.ToArray().Select(x => (float) x));
				ovb.AddRange(n.ToArray().Select(x => (float) x));
				ovb.AddRange(t.ToArray().Select(x => (float) x));
				return ind;
			}

			foreach(var (a, b, c) in polys) {
				oib.Add(Add(a));
				oib.Add(Add(c));
				oib.Add(Add(b));
			}

			return (ovb.ToArray(), oib.ToArray());
		}
	}
}