using OpenEQ.Network;
using OpenEQ.Views;
using System.Collections.Generic;
using static System.Console;

namespace OpenEQ.Controllers {
	class ZoneController : BaseController<ZoneStream, ZoneView> {
		public static ZoneController Instance = new ZoneController();

		public ZoneServerInfo TargetServer;
		public ZoneNumber TargetZone;

		protected override ZoneStream InitializeConnection() {
			View.NewZone(TargetZone);
			var conn = new ZoneStream(TargetServer.Host, TargetServer.Port, WorldController.Instance.CurrentCharacter.Name);
			conn.Spawned += (_, mob) => {
				if(mob.Name == WorldController.Instance.CurrentCharacter.Name)
					View.PlayerPositionHeading = mob.Position.GetPositionHeading();
			};
			return conn;
		}
	}
}
