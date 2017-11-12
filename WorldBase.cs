using Godot;
using OpenEQ;
using System;
using System.Linq;
using System.Collections.Generic;
using static System.Console;
using System.Threading.Tasks;

public class WorldBase : Node
{
	List<Tuple<SpatialMaterial, Texture[], float>> Animat;
	Dictionary<uint, Tuple<float, float, float, float>> Moving;
	Dictionary<uint, MobModel> mdict;

	public override void _Ready()
    {
		var pos = LogicBridge.Instance.CharacterSpawnPosition;
		this.GetNode<Spatial>("../RigidBody").Translation = pos.XYZ() * new Vector3(1, 1, -1);
		WriteLine($"Starting off at {this.GetNode<Spatial>("../RigidBody").Translation}");
		var zone = LogicBridge.Instance.CurZone;
		WriteLine($"Loading zone {zone}");
		ZoneReader.Read(this, System.IO.File.OpenRead($@"c:\aaa\projects\openeq\converter\{ zone }.zip"), out Animat);

		var orc = MobModel.Read(System.IO.File.OpenRead($@"c:\aaa\projects\openeq\converter\orc_chr.zip"));

		mdict = new Dictionary<uint, MobModel>();
		Moving = new Dictionary<uint, Tuple<float, float, float, float>>();
		foreach(var spawn in LogicBridge.Instance.Spawns) {
			var o = orc.Instantiate();
			mdict[spawn.SpawnID] = o;
			AddChild(o.Node);
			//o.StartAnimation("L01");
			var ph = spawn.Position.GetPositionHeading();
			WriteLine($"Spawn at {ph.Item1} {ph.Item2} {ph.Item3}");
			o.Node.Transform = new Transform(new Basis(new Vector3(1, 0, 0), new Vector3(0, 1, 0), new Vector3(0, 0, 1)), ph.XYZ() * new Vector3(1, 1, -1));
			o.Node.RotateY(ph.Item4 * 2f * Mathf.PI);
			o.Node.Hide();
		}

		LogicBridge.Instance.OnMoved += (_, pu) => {
			if(!mdict.ContainsKey(pu.ID)) {
				WriteLine($"Unknown mob update: {pu.ID}");
				return;
			}
			if(pu.Position.Heading > 255 * 8 || pu.Position.Animation != 0 || pu.Position.DeltaX != 0 || pu.Position.DeltaY != 0 || pu.Position.DeltaZ != 0 || pu.Position.DeltaHeading != 0) {
				WriteLine(pu);
				Moving[pu.ID] = pu.Position.GetDeltas();
			} else if(Moving.ContainsKey(pu.ID))
				Moving.Remove(pu.ID);
			var o = mdict[pu.ID];
			var ph = pu.Position.GetPositionHeading();
			o.Node.Transform = new Transform(new Basis(new Vector3(1, 0, 0), new Vector3(0, 1, 0), new Vector3(0, 0, 1)), ph.XYZ() * new Vector3(1, 1, -1));
			o.Node.RotateY(ph.Item4 * 2f * Mathf.PI);
			o.Node.Show();
		};

		this.GetNode<Spatial>("../RigidBody/CameraHolder").RotateY(pos.Item4 * 2f * Mathf.PI);
	}

	public override void _Process(float delta)
    {
		if(Animat == null)
			return;
		var ticks = OS.GetTicksMsec();
		foreach(var elem in Animat)
			elem.Item1.AlbedoTexture = elem.Item2[(int) Mathf.round(ticks / elem.Item3) % elem.Item2.Length];
		foreach(var elem in Moving) {
			var o = mdict[elem.Key];
			var d = elem.Value;
			//WriteLine($"Attempting to move with delta vector {d.XYZ() * new Vector3(1, 1, -1)}");
			o.Node.Transform = new Transform(o.Node.Transform.basis, o.Node.Transform.origin + d.XYZ() * new Vector3(1, 1, -1) * delta * 5);
			//o.Node.RotateY(d.Item4 * 2f * Mathf.PI * delta * 2);
		}
		var rb = this.GetNode<Spatial>("../RigidBody");
		var pos = rb.Translation;
		LogicBridge.Instance.UpdatePosition(new Tuple<float, float, float, float>(pos.x, pos.y, pos.z, 0));
	}
}
