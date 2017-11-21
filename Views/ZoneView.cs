using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using static System.Console;
using Godot;
using OpenEQ.Controllers;
using OpenEQ.Network;

namespace OpenEQ.Views {
	class ZoneView : Node {
		ZoneController Controller = ZoneController.Instance;

		ZoneNumber CurrentZone = ZoneNumber.Unknown;
		Spatial ZoneNode;

		List<Tuple<SpatialMaterial, Godot.Texture[], float>> Animat;

		[ControlGetter] RigidBody RigidBody = null;
		[ControlGetter("RigidBody/CameraHolder")] Spatial CameraHolder = null;

		public Tuple<float, float, float, float> PlayerPositionHeading {
			set {
				RigidBody.Translation = value.XYZ() * new Vector3(1, 1, -1);
				CameraHolder.RotateY(value.Item4 * 2f * Mathf.PI);
			}
		}

		public override void _Ready() {
			Controller.Register(this);
			Controller.Connect();

			/*
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
			 */
		}

		public override void _Process(float delta) {
			if(Animat == null)
				return;

			var ticks = OS.GetTicksMsec();
			foreach(var elem in Animat)
				elem.Item1.AlbedoTexture = elem.Item2[(int) Mathf.round(ticks / elem.Item3) % elem.Item2.Length];
		}

		public void NewZone(ZoneNumber zone) {
			if(zone == CurrentZone)
				return;
			if(CurrentZone != ZoneNumber.Unknown)
				Cleanup();

			CurrentZone = zone;
			ZoneNode = new Spatial();
			ZoneNode.Visible = false;
			AddChild(ZoneNode);
			//AsyncHelper.Run(() => {
				ZoneReader.Read(ZoneNode, System.IO.File.OpenRead($@"c:\aaa\projects\openeq\converter\{ zone }.zip"), out Animat);
				WriteLine("Loaded zone?!");
				ZoneNode.Visible = true;
			//});
		}

		void Cleanup() {
			RemoveChild(ZoneNode);
			ZoneNode = null;
		}
	}
}
