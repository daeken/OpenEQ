using OpenEQ.Network;
using OpenEQ.Views;
using System.Collections.Generic;
using static System.Console;

namespace OpenEQ.Controllers {
	class WorldController : BaseController<WorldStream, WorldView> {
		public static WorldController Instance = new WorldController();

		public ServerListElement LoggingIn;
		List<CharacterSelectEntry> Characters;
		public CharacterSelectEntry CurrentCharacter;

		protected override WorldStream InitializeConnection() {
			var lc = LoginController.Instance;
			var conn = new WorldStream(LoggingIn.WorldIP, 9000, lc.Connection.accountID, lc.Connection.sessionKey);
			conn.CharacterList += (_, chars) => {
				Characters = chars;
				View.ShowCharacters(chars);
			};
			conn.ZoneServer += (_, server) => {
				ZoneController.Instance.TargetServer = server;
				View.LoadZoneScene();
			};
			return conn;
		}

		public void SelectCharacter(int index) {
			var c = CurrentCharacter = Characters[index];
			ZoneController.Instance.TargetZone = (ZoneNumber) c.Zone;
			Connection.SendEnterWorld(new EnterWorld(c.Name, false, false));
		}
	}
}
