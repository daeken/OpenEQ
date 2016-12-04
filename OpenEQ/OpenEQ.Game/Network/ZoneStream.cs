using System;
using System.Collections.Generic;
using System.Text;
using static System.Console;
using static OpenEQ.Network.Utility;

namespace OpenEQ.Network {
    public class ZoneStream : EQStream {
        string charName;

        public ZoneStream(string host, int port, string charName) : base(host, port) {
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
            switch((ZoneOp) packet.Opcode) {
                case ZoneOp.PlayerProfile:
                    var player = packet.Get<PlayerProfile>();
                    //WriteLine(player);
                    break;

                case ZoneOp.CharInventory:
                    var inventory = packet.Get<CharInventory>();
                    //WriteLine(inventory);
                    break;

                case ZoneOp.TimeOfDay:
                    var timeofday = packet.Get<TimeOfDay>();
                    //WriteLine(timeofday);
                    break;

                case ZoneOp.TaskActivity:
                    var activity = packet.Get<TaskActivity>();
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
                    WriteLine(mob);
                    break;

                case ZoneOp.SendFindableNPCs:
                    var npc = packet.Get<FindableNPC>();
                    //WriteLine(npc);
                    break;

                default:
                    WriteLine($"Unhandled packet in ZoneStream: {(ZoneOp) packet.Opcode} (0x{packet.Opcode:X04})");
                    Hexdump(packet.Data);
                    break;
            }
        }
    }
}
