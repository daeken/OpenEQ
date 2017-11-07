using System.Collections.Generic;
using System.IO;
using ICSharpCode.SharpZipLib.Zip;
using System.Linq;
using Godot;
using static System.Console;
using System;

class ZoneReader {
	public static void Read(Node node, Stream fs, out List<Tuple<SpatialMaterial, Texture[], float>> animat) {
		var zip = new ZipFile(fs);
		var zonefile = zip.GetInputStream(zip.GetEntry("zone.oez"));
		var reader = new BinaryReader(zonefile);

		var nummats = reader.ReadInt32();
		var materials = new Dictionary<int, SpatialMaterial>();
		var hidden = new Dictionary<int, bool>();
		animat = new List<Tuple<SpatialMaterial, Texture[], float>>();
		for(var i = 0; i < nummats; ++i) {
			var flags = reader.ReadUInt32();
			var param = reader.ReadUInt32();
			var numtex = reader.ReadUInt32();
			var textures = new Texture[numtex];
			for(var j = 0; j < numtex; ++j) {
				var fn = reader.ReadString();
				var entry = zip.GetEntry(fn);
				textures[j] = TextureLoader.Load(zip.GetInputStream(entry), (int) entry.Size, flags != 1);
			}
			hidden[i] = flags == 4;
			var mat = materials[i] = new SpatialMaterial();
			if(numtex > 1)
				animat.Add(new Tuple<SpatialMaterial, Texture[], float>(mat, textures, param));
			else if(numtex == 1)
				mat.AlbedoTexture = textures[0];
			if(flags == 1)
				mat.ParamsUseAlphaScissor = true;
			else if(flags != 0)
				mat.FlagsTransparent = true;
		}

		var objects = new List<ArrayMesh>();
		var numobjs = reader.ReadUInt32();
		WriteLine($"Num objs: {numobjs}");
		for(var i = 0; i < numobjs; ++i) {
			var obj = new ArrayMesh();
			objects.Add(obj);
			var matoffs = new List<int>();

			var nummeshes = reader.ReadUInt32();
			WriteLine($"Num meshes: {nummeshes}");
			var surfid = 0;
			for(var j = 0; j < nummeshes; ++j) {
				var matid = reader.ReadInt32();
				var numvert = reader.ReadInt32();
				var arrays = new object[ArrayMesh.ARRAY_MAX];
				arrays[ArrayMesh.ARRAY_VERTEX] = Enumerable.Range(0, numvert).Select(_ => reader.ReadVector3()).ToArray();
				arrays[ArrayMesh.ARRAY_NORMAL] = Enumerable.Range(0, numvert).Select(_ => reader.ReadVector3()).ToArray();
				arrays[ArrayMesh.ARRAY_TEX_UV] = Enumerable.Range(0, numvert).Select(_ => reader.ReadVector2()).ToArray();
				var numpoly = reader.ReadInt32();
				var ind = Enumerable.Range(0, numpoly * 3).Select(_ => (int) reader.ReadUInt32()).ToArray();
				arrays[ArrayMesh.ARRAY_INDEX] = ind;
				//var collidable = Enumerable.Range(0, numpoly).Select(_ => reader.ReadUInt32() == 1).ToArray();
				
				if(hidden[matid])
					continue;

				obj.AddSurfaceFromArrays(VisualServer.PRIMITIVE_TRIANGLES, arrays);
				obj.SurfaceSetMaterial(surfid++, materials[matid]);
			}
		}

		var mi = new MeshInstance { Mesh = objects[0] };
		mi.CreateTrimeshCollision();
		node.AddChild(mi);

		var oset = new List<Transform>[objects.Count - 1];
		for(var i = 0; i < oset.Length; ++i)
			oset[i] = new List<Transform>();
		var numplace = reader.ReadUInt32();
		for(var i = 0; i < numplace; ++i) {
			var ind = reader.ReadInt32();
			WriteLine($"Placeable {i} has index {ind} / {objects.Count}");
			var pos = reader.ReadVector3();
			var rot = reader.ReadVector3();
			var size = reader.ReadVector3();
			var quat = rot.EulerToQuat();
			var trans = new Transform(new Basis(
				quat.xform(new Vector3(1, 0, 0)) * size.x, 
				quat.xform(new Vector3(0, 1, 0)) * size.y, 
				quat.xform(new Vector3(0, 0, 1)) * size.z
			), pos);
			oset[ind - 1].Add(trans);
		}
		for(var i = 1; i < numobjs; ++i) {
			var set = oset[i - 1];
			if(set.Count == 0)
				continue;
			else if(set.Count < 100) {
				foreach(var trans in set) {
					mi = new MeshInstance() { Mesh = objects[i], Transform = trans };
					mi.CreateTrimeshCollision();
					node.AddChild(mi);
				}
			} else {
				var mmesh = new MultiMesh {
					TransformFormat = MultiMesh.TRANSFORM_3D,
					Mesh = objects[i],
					InstanceCount = set.Count
				};
				node.AddChild(new MultiMeshInstance() { Multimesh = mmesh });
				WriteLine($"Placeable object {i} has {mmesh.InstanceCount} instances");
				for(var j = 0; j < set.Count; ++j) {
					mmesh.SetInstanceTransform(j, set[j]);
				}
			}
		}
	}
}
