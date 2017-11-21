using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using static System.Console;
using Godot;
using OpenEQ.Network;
using OpenEQ.Views;

namespace OpenEQ.Controllers {
	class LoginController : BaseController<LoginStream, LoginView> {
		public static LoginController Instance = new LoginController();

		public Tuple<string, int> Server { get; set; } = new Tuple<string, int>("127.0.0.1", 5999);
		List<ServerListElement> ServerList;

		protected override LoginStream InitializeConnection() {
			//EQStream.Debug = true;

			var conn = new LoginStream(Server.Item1, Server.Item2);
			conn.LoginSuccess += (_, success) => {
				if(success)
					conn.RequestServerList();
				else
					View.LoginError = "Login failed.";
			};
			conn.ServerList += (_, servers) => {
				ServerList = servers;
				View.ShowServers(servers);
			};
			conn.PlaySuccess += (_, server) => {
				if(server == null) {
					// XXX: Handle the failure case
					return;
				}
				WorldController.Instance.LoggingIn = server.Value;
				View.LoadWorldScene();
			};
			return conn;
		}

		public void StartLogin() {
			RequireView();
			Connection.Login(View.Username, View.Password);
		}

		public void SelectServer(int index) {
			Connection.Play(ServerList[index]);
		}
	}
}
