using Godot;
using OpenEQ;
using System;
using System.Collections.Generic;
using static System.Console;

public class WorldBase : Node
{
	List<Tuple<SpatialMaterial, Texture[], float>> Animat;

	public override void _Ready()
    {
		var pos = LogicBridge.Instance.CharacterSpawnPosition;
		((Spatial) GetNode("../RigidBody")).Translation = new Vector3(pos.Item1, pos.Item2, -pos.Item3);
		WriteLine($"Starting off at {((Spatial) GetNode("../RigidBody")).Translation}");
		var zone = LogicBridge.Instance.CurZone;
		WriteLine($"Loading zone {zone}");
		ZoneReader.Read(this, System.IO.File.OpenRead($@"c:\aaa\projects\oldopeneq\converter\{ zone }.zip"), out Animat);
		((Spatial) GetNode("../RigidBody/CameraHolder")).RotateY(pos.Item4 * Mathf.PI / 180f); 
    }

    public override void _Process(float delta)
    {
		var ticks = OS.GetTicksMsec();
		foreach(var elem in Animat)
			elem.Item1.AlbedoTexture = elem.Item2[(int) Mathf.round(ticks / elem.Item3) % elem.Item2.Length];
    }
}
