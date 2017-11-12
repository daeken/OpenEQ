using OpenEQ.Network;
using System;
using System.Collections.Generic;
using static System.Console;

namespace OpenEQ {
	class LogicBridge {
		public static LogicBridge Instance = new LogicBridge();

		// Login
		public event EventHandler<bool> OnLoginResult;
		public event EventHandler<List<ServerListElement>> OnServerList;
		public event EventHandler<bool> OnPlayResult;

		// World
		public event EventHandler<List<CharacterSelectEntry>> OnCharacterList;

		// Zone
		public event EventHandler<bool> OnCharacterSpawn;
		public event EventHandler<Spawn> OnSpawn;
		public event EventHandler<PlayerPositionUpdate> OnMoved;

		public List<Spawn> Spawns = new List<Spawn>();

		LoginStream login;
		WorldStream world;
		ZoneStream zone;
		ServerListElement curWorld;
		CharacterSelectEntry curChar;

		public ZoneNumber CurZone = ZoneNumber.gfaydark;
		public Tuple<float, float, float, float> CharacterSpawnPosition = new Tuple<float, float, float, float>(2678 / 8f, 632 / 8f, 2135 / 8f, 1767 / 8f);

		public LogicBridge() {
			//EQStream.Debug = true;

			login = new LoginStream("127.0.0.1", 5999);
			login.LoginSuccess += (_, success) => OnLoginResult(this, success);
			login.ServerList += (_, servers) => OnServerList(this, servers);
			login.PlaySuccess += (_, server) => {
				if(server != null)
					curWorld = (ServerListElement) server;
				OnPlayResult(this, server != null);
			};
		}

		public void Login(string username, string password) {
			login.Login(username, password);
		}

		public void RequestServerList() {
			login.RequestServerList();
		}

		public void Play(ServerListElement server) {
			login.Play(server);
		}

		public void ConnectToWorld() {
			world = new WorldStream(curWorld.WorldIP, 9000, login.accountID, login.sessionKey);
			world.CharacterList += (_, chars) => OnCharacterList(this, chars);
			world.ZoneServer += (_, server) => {
				zone = new ZoneStream(server.Host, server.Port, curChar.Name);
				zone.Spawned += (__, mob) => {
					if(mob.Name == curChar.Name) {
						CharacterSpawnPosition = mob.Position.GetPositionHeading();
						OnCharacterSpawn?.Invoke(this, true);
					} else
						Spawns.Add(mob);
				};
				zone.PositionUpdated += (__, pu) => {
					OnMoved?.Invoke(this, pu);
				};
			};
		}

		public void EnterWorld(CharacterSelectEntry character, bool tutorial = false, bool goHome = false) {
			curChar = character;
			CurZone = (ZoneNumber) character.Zone;
			world.SendEnterWorld(new EnterWorld(character.Name, tutorial, goHome));
		}

		public void UpdatePosition(Tuple<float, float, float, float> pos) {
			zone.UpdatePosition(pos);
		}
	}
}
