using System;
using System.Collections.Generic;
using System.Text;
using static System.Console;
using static OpenEQ.Netcode.Utility;

namespace OpenEQ.Netcode {
	public class ZoneStream : EQStream {
		readonly string CharName;
		bool Entering = true;
		ushort PlayerSpawnId;
		ushort UpdateSequence;

		public event EventHandler<Spawn> Spawned;
		public event EventHandler<PlayerPositionUpdate> PositionUpdated;

		public ZoneStream(string host, int port, string charName) : base(host, port) {
			SendKeepalives = true;
			CharName = charName;

			WriteLine("Starting zone connection...");
			Connect();
			SendSessionRequest();
		}

		protected override void HandleSessionResponse(Packet packet) {
			Send(packet);

			Send(AppPacket.Create(ZoneOp.ZoneEntry, new ClientZoneEntry(CharName)));
		}

		protected override void HandleAppPacket(AppPacket packet) {
			//WriteLine($"Zone app packet: {(ZoneOp) packet.Opcode}");
			switch((ZoneOp) packet.Opcode) {
				case ZoneOp.PlayerProfile:
					var player = packet.Get<PlayerProfile>();
					//WriteLine(player);
					break;

				case ZoneOp.TimeOfDay:
					var timeofday = packet.Get<TimeOfDay>();
					//WriteLine(timeofday);
					break;

				case ZoneOp.TaskActivity:
					// XXX: Handle short activities!
					//var activity = packet.Get<TaskActivity>();
					//WriteLine(activity);
					break;

				case ZoneOp.TaskDescription:
					var desc = packet.Get<TaskDescription>();
					//WriteLine(desc);
					break;

				case ZoneOp.CompletedTasks:
					var comp = packet.Get<CompletedTasks>();
					//WriteLine(comp);
					break;

				case ZoneOp.XTargetResponse:
					var xt = packet.Get<XTarget>();
					//WriteLine(xt);
					break;

				case ZoneOp.Weather:
					var weather = packet.Get<Weather>();
					//WriteLine(weather);

					if(Entering)
						Send(AppPacket.Create(ZoneOp.ReqNewZone));
					break;

				case ZoneOp.TributeTimer:
					var timer = packet.Get<TributeTimer>();
					//WriteLine(timer);
					break;

				case ZoneOp.TributeUpdate:
					var update = packet.Get<TributeInfo>();
					//WriteLine(update);
					break;

				case ZoneOp.ZoneEntry:
					var mob = packet.Get<Spawn>();
					if(mob.Name == CharName)
						PlayerSpawnId = (ushort) mob.SpawnID;
					Spawned?.Invoke(this, mob);
					break;

				case ZoneOp.NewZone:
					Send(AppPacket.Create(ZoneOp.ReqClientSpawn));
					break;

				case ZoneOp.SendExpZonein:
					if(Entering) {
						Send(AppPacket.Create(ZoneOp.ClientReady));
						Entering = false;
					}

					break;

				case ZoneOp.CharInventory:
					break;

				case ZoneOp.SendFindableNPCs:
					var npc = packet.Get<FindableNPC>();
					//WriteLine(npc);
					break;

				case ZoneOp.ClientUpdate:
					var pu = packet.Get<PlayerPositionUpdate>();
					PositionUpdated?.Invoke(this, pu);
					break;

				case ZoneOp.HPUpdate:
					break;
				
				case ZoneOp.SpawnDoor:
					for(var i = 0; i < packet.Data.Length; i += 92) {
						var door = new Door(packet.Data, i);
						WriteLine(door);
					}
					break;

				default:
					WriteLine($"Unhandled packet in ZoneStream: {(ZoneOp) packet.Opcode} (0x{packet.Opcode:X04})");
					Hexdump(packet.Data);
					break;
			}
		}

		public void UpdatePosition(Tuple<float, float, float, float> Position) {
			var update = new ClientPlayerPositionUpdate {
				ID = PlayerSpawnId,
				Sequence = UpdateSequence++,
				X = Position.Item1,
				Y = Position.Item2,
				Sub1 = new ClientUpdatePositionSub1(),
				Z = Position.Item3,
				Sub2 = new ClientUpdatePositionSub2(0, (ushort) (Position.Item4 * 8f * 255f))
			};
			Send(AppPacket.Create(ZoneOp.ClientUpdate, update));
		}
	}
}