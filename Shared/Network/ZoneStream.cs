using System;
using System.Collections.Generic;
using System.Text;
using static System.Console;
using static OpenEQ.Network.Utility;

namespace OpenEQ.Network {
    public class ZoneStream : EQStream {
        string charName;
        bool entering = true;
		bool done = false;
		ushort playerSpawnId;
		ushort updateSequence = 0;

		public event EventHandler<Spawn> Spawned;
		public event EventHandler<PlayerPositionUpdate> PositionUpdated;

        public ZoneStream(string host, int port, string charName) : base(host, port) {
            SendKeepalives = true;
            this.charName = charName;

            WriteLine("Starting zone connection...");
            Connect();
            SendSessionRequest();
        }

        protected override void HandleSessionResponse(Packet packet) {
            Send(packet);

            Send(AppPacket.Create(ZoneOp.ZoneEntry, new ClientZoneEntry(charName)));
        }

        protected override void HandleAppPacket(AppPacket packet) {
			WriteLine($"Zone app packet: {(ZoneOp) packet.Opcode}");
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

                    if(entering)
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
					if(mob.Name == charName)
						playerSpawnId = (ushort) mob.SpawnID;
					Spawned(this, mob);
                    break;

                case ZoneOp.NewZone:
                    Send(AppPacket.Create(ZoneOp.ReqClientSpawn));
                    break;

				case ZoneOp.SendExpZonein:
					if(entering) {
						Send(AppPacket.Create(ZoneOp.ClientReady));
						entering = false;
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

                default:
                    WriteLine($"Unhandled packet in ZoneStream: {(ZoneOp) packet.Opcode} (0x{packet.Opcode:X04})");
                    Hexdump(packet.Data);
                    break;
            }
        }

		public void UpdatePosition(Tuple<float, float, float, float> Position) {
			var update = new ClientPlayerPositionUpdate();
			update.ID = playerSpawnId;
			update.Sequence = updateSequence++;
			update.X = Position.Item1;
			update.Y = Position.Item2;
			update.Sub1 = new ClientUpdatePositionSub1();
			update.Z = Position.Item3;
			update.Sub2 = new ClientUpdatePositionSub2(0, (ushort) (Position.Item4 * 8f * 255f));
			Send(AppPacket.Create(ZoneOp.ClientUpdate, update));
		}
    }
}
