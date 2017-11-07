using System;
using System.Linq;
using OpenEQ.Network;
using System.Threading;
using static System.Console;

namespace OpenEQ.NetClient {
	class Program {
		static void Main(string[] args) {
			var _waitForLogin = new AutoResetEvent(false);

			EQStream.Debug = true;

			var running = true;
			var login = new LoginStream("127.0.0.1", 5999);

			login.LoginSuccess += (sender, success) => {
				if(success) {
					WriteLine("Login successful. Requesting server list.");
					login.RequestServerList();
				} else {
					WriteLine("Login failed.");
					running = false;
				}
			};

			login.ServerList += (sender, servers) => {
				WriteLine($"Got {servers.Count} servers:");
				foreach(var server in servers) {
					WriteLine(
						$"- '{server.Longname}' @ {server.WorldIP} is {server.Status} with {server.PlayersOnline} players");
				}

				var chosen = servers.First(s => s.Longname.Contains("Ex1Gj0"));
				//var chosen = servers[0];
				WriteLine($"Sending play request for server '{chosen.Longname}' @ {chosen.WorldIP}");
				login.Play(chosen);
			};

			login.PlaySuccess += (sender, server) => {
				if(server == null) {
					WriteLine("Play request rejected.");
					running = false;
				} else {
					WriteLine("Play request accepted.  Initializing world.");
					var world = new WorldStream(server?.WorldIP, 9000, login.accountID, login.sessionKey);
					SetupWorld(world);
				}
			};

			login.Login("daeken", "omgwtfbbq");

			//_waitForLogin.WaitOne();
			while(running)
				Thread.Sleep(100);

			WriteLine("Completed.");
			ReadLine();
		}

		static void SetupWorld(WorldStream world) {
			string charname = null;
			world.CharacterCreateNameApproval += (sender, approvalState) => {
				if(1 == approvalState) {
					// approved.  Send charactercreate and EnterWorld.
					world.SendCharacterCreate(new CharCreate {
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

					world.SendEnterWorld(new EnterWorld {
						Name = charname,
						GoHome = false,
						Tutorial = false
					});
				} else {
					// Denied.  Let them try a different name.
				}
			};

			world.CharacterList += (sender, chars) => {
				WriteLine($"Got {chars.Count} characters:");

				if(0 == chars.Count) {
					charname = "JimTomShiner";

					Console.WriteLine("0 Characters found.  Simple char creation:");
					Console.WriteLine($"1. Barbarian, Male, Warrior, Name is {charname}");
					Console.WriteLine("Enter number of the character type you want and press Enter.");
					var selection = Console.ReadLine();

					// Send char create packet with defaults.
					world.SendNameApproval(new NameApproval {
						Name = charname,
						Class = 3,
						Race = 1,
						Unknown = 214
					});
				} else {
					foreach(var character in chars)
						WriteLine($"- {character.Name} - Level {character.Level}");

					Console.WriteLine("Enter a character number and press Enter.");
					var charNum = "1";//Console.ReadLine();
					charname = chars[Convert.ToInt32(charNum) - 1].Name;
					WriteLine(chars[0]);
					//Environment.Exit(0);
					WriteLine($"Entering world with {charname}");
					//world.ResetAckForZone();
					world.SendEnterWorld(new EnterWorld {
						Name = charname,
						GoHome = false,
						Tutorial = false
					});
				}
			};

			world.ZoneServer += (sender, server) => {
				WriteLine($"Zone server info: {server.Host}:{server.Port}");

				var zone = new ZoneStream(server.Host, server.Port, charname);
				SetupZone(zone);
			};
		}

		static void SetupZone(ZoneStream zone) {
			zone.Spawned += (_, mob) => {
				if(mob.Name.Contains("Jim")) {
					WriteLine(mob);
					Environment.Exit(0);
				}
			};
		}
	}
}