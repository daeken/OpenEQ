using System.Threading.Tasks;
using MoreLinq;
using OpenEQ.Netcode;
using static System.Console;

namespace NetClient {
	internal class Program {
		static string Input(string prompt) {
			Write($"{prompt} > ");
			return ReadLine().TrimEnd();
		}
		
		static void Main(string[] args) {
			while(true) {
				var loginStream = new LoginStream(args[0], int.Parse(args[1]));
				loginStream.LoginSuccess += (_, success) => {
					if(success) {
						WriteLine($"Login succeeded (accountID={loginStream.AccountID}).  Requesting server list");
						loginStream.RequestServerList();
					} else {
						WriteLine("Login failed");
						loginStream.Disconnect();
					}
				};

				loginStream.ServerList += (_, servers) => {
					servers.ForEach((serv, i) =>
						WriteLine($"{i + 1}: {serv.Longname} ({serv.PlayersOnline} players online)"));
					int ret;
					while(!int.TryParse(Input("Server number"), out ret) || ret < 1 || servers.Count < ret) {}
					loginStream.Play(servers[ret - 1]);
				};
				
				loginStream.PlaySuccess += (_, server) => {
					if(server == null) {
						WriteLine("Failed to connect to server.  Try everything again.");
						loginStream.Disconnect();
						return;
					}
					
					ConnectWorld(loginStream, server.Value);
				};

				loginStream.Login(Input("Username"), Input("Password"));

				while(!loginStream.Disconnecting)
					Task.Delay(100).Wait();
			}
		}

		static void ConnectWorld(LoginStream ls, ServerListElement server) {
			WriteLine($"Selected {server}.  Connecting.");
			var worldStream = new WorldStream(server.WorldIP, 9000, ls.AccountID, ls.SessionKey);
			
			string charName = null;
			worldStream.CharacterList += (_, chars) => {
				WriteLine("Select a character:");
				WriteLine("0: Create a new character");
				chars.ForEach((@char, i) => WriteLine($"{i + 1}: {@char.Name} - Level {@char.Level}"));
				int ret;
				while(!int.TryParse(Input("Character number"), out ret) || ret < 0 || chars.Count < ret) {}
				if(ret == 0)
					CreateCharacter();
				else
					worldStream.SendEnterWorld(new EnterWorld(charName = chars[ret - 1].Name, false, false));
			};

			void CreateCharacter() {
				charName = Input("Name");
				worldStream.SendNameApproval(new NameApproval {
					Name = charName, 
					Class = 3, 
					Race = 1, 
					Unknown = 214
				});
			}

			worldStream.CharacterCreateNameApproval += (_, success) => {
				if(!success) {
					WriteLine("Name not approved by server");
					CreateCharacter();
				} else {
					WriteLine("Name approved, creating");
					worldStream.SendCharacterCreate(new CharCreate {
						Class_ = 1,
						Haircolor = 255,
						BeardColor = 255,
						Beard = 255,
						Gender = 0,
						Race = 2,
						StartZone = 29,
						HairStyle = 255,
						Deity = 211,
						STR = 113,
						STA = 130,
						AGI = 87,
						DEX = 70,
						WIS = 70,
						INT = 60,
						CHA = 55,
						Face = 0,
						EyeColor1 = 9,
						EyeColor2 = 9,
						DrakkinHeritage = 1,
						DrakkinTattoo = 0,
						DrakkinDetails = 0,
						Tutorial = 0
					});
				}
			};

			worldStream.ZoneServer += (_, zs) => {
				WriteLine($"Got zone server at {zs.Host}:{zs.Port}.  Connecting");
				ConnectZone(charName, zs.Host, zs.Port);
			};
		}

		static void ConnectZone(string charName, string host, ushort port) {
			var zoneStream = new ZoneStream(host, port, charName);
			zoneStream.Spawned += (_, mob) => {
				//WriteLine($"Spawn {mob.Name}");
			};
			zoneStream.PositionUpdated += (_, update) => {
				//WriteLine($"Position updated: {update.ID} {update.Position}");
			};
		}
	}
}