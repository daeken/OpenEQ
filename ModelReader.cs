using Godot;
using ICSharpCode.SharpZipLib.Zip;
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using static System.Console;

class ModelReader : Godot.Object {
	public static void Read(Node node, Stream fs) {
		var zip = new ZipFile(fs);
		var zonefile = zip.GetInputStream(zip.GetEntry("ORC_ACTORDEF.oec"));
		var reader = new BinaryReader(zonefile);

		var obj = new ArrayMesh();
		var nummeshes = reader.ReadInt32();
		for(var i = 0; i < nummeshes; ++i) {
			var flags = reader.ReadUInt32();
			var numtex = reader.ReadUInt32();
			var textures = new Texture[numtex];
			for(var j = 0; j < numtex; ++j) {
				var fn = reader.ReadString();
				var entry = zip.GetEntry(fn);
				textures[j] = TextureLoader.Load(zip.GetInputStream(entry), (int) entry.Size, flags != 1);
			}
			var mat = new SpatialMaterial();
			if(numtex > 0)
				mat.AlbedoTexture = textures[0];
			if(flags == 1)
				mat.ParamsUseAlphaScissor = true;
			//else if(flags != 0)
			//	mat.FlagsTransparent = true;

			var numvert = reader.ReadInt32();
			var arrays = new object[ArrayMesh.ARRAY_MAX];
			arrays[ArrayMesh.ARRAY_VERTEX] = Enumerable.Range(0, numvert).Select(_ => reader.ReadVector3()).ToArray();
			arrays[ArrayMesh.ARRAY_NORMAL] = Enumerable.Range(0, numvert).Select(_ => reader.ReadVector3()).ToArray();
			arrays[ArrayMesh.ARRAY_TEX_UV] = Enumerable.Range(0, numvert).Select(_ => reader.ReadVector2()).ToArray();
			arrays[ArrayMesh.ARRAY_BONES] = Enumerable.Range(0, numvert * 4).Select(o => (o & 3) == 0 ? reader.ReadUInt32() : 0f).ToArray();
			arrays[ArrayMesh.ARRAY_WEIGHTS] = Enumerable.Range(0, numvert * 4).Select(o => (o & 3) == 0 ? 1f : 0f).ToArray();
			var numpoly = reader.ReadInt32();
			var ind = Enumerable.Range(0, numpoly * 3).Select(_ => (int) reader.ReadUInt32()).ToArray();
			arrays[ArrayMesh.ARRAY_INDEX] = ind;
			obj.AddSurfaceFromArrays(VisualServer.PRIMITIVE_TRIANGLES, arrays);
			obj.SurfaceSetMaterial(i, mat);
		}

		var numbones = reader.ReadUInt32();
		var skel = new Skeleton();
		node.AddChild(skel);
		var spath = skel.GetPath();
		for(var i = 0; i < numbones; ++i) {
			skel.AddBone($"bone_{i}");
			skel.SetBoneParent(i, reader.ReadInt32());
		}

		var numanim = reader.ReadUInt32();
		var aniplayer = new AnimationPlayer();
		for(var i = 0; i < numanim; ++i) {
			var name = reader.ReadString();
			var ani = new Animation();
			var boneframes = new List<Tuple<Vector3, Quat>>[numbones];
			for(var j = 0; j < numbones; ++j) {
				ani.AddTrack(Animation.TYPE_TRANSFORM);
				ani.TrackSetPath(j, spath + $":bone_{j}");
				var numframes = reader.ReadUInt32();
				for(var k = 0; k < numframes; ++k)
					ani.TransformTrackInsertKey(j, 0.1f * k, reader.ReadVector3(), reader.ReadQuat(), new Vector3(1, 1, 1));
			}
			aniplayer.AddAnimation(name, ani);
			WriteLine(name);
		}

		aniplayer.SetActive(true);
		aniplayer.Play("");
		node.AddChild(aniplayer);
		aniplayer.Connect("animation_finished", new ModelReader(aniplayer), "AnimationFinished");

		var mi = new MeshInstance { Mesh = obj };
		skel.AddChild(mi);
		skel.RotateX(-Mathf.PI / 2);
	}

	AnimationPlayer player;
	int count = 0;
	ModelReader(AnimationPlayer player) {
		this.player = player;
	}

	void AnimationFinished(string ani) {
		var al = player.GetAnimationList();
		var next = al[count++ % al.Length];
		WriteLine($"Playing animation '{next}'");
		player.Play(next);
	}
}
