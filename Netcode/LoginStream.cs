using System;
using System.Collections.Generic;
using static System.Console;
using static System.Text.Encoding;
using static OpenEQ.Netcode.Utility;

namespace OpenEQ.Netcode {
	public class LoginStream : EQStream {
		public event EventHandler<bool> LoginSuccess;
		public event EventHandler<List<ServerListElement>> ServerList;
		public event EventHandler<ServerListElement?> PlaySuccess;

		public uint AccountID;
		public string SessionKey;

		ServerListElement? CurPlay;

		byte[] CryptoBlob;

		public LoginStream(string host, int port) : base(host, port) => Connect();

		public void Login(string username, string password) {
			CryptoBlob = Encrypt(username, password);
			SendSessionRequest();
		}

		byte[] Encrypt(string username, string password) {
			var tbuf = new byte[username.Length + password.Length + 2];
			Array.Copy(ASCII.GetBytes(username), tbuf, username.Length);
			Array.Copy(ASCII.GetBytes(password), 0, tbuf, username.Length + 1, password.Length);
			tbuf[username.Length] = 0;
			tbuf[username.Length + password.Length + 1] = 0;
			return Encrypt(tbuf);
		}

		byte[] Encrypt(byte[] buffer) {
			var empty = new byte[] { 0, 0, 0, 0, 0, 0, 0, 0 };
			var des = new DESCrypto(empty, empty);
			return des.Encrypt(buffer);
		}

		byte[] Decrypt(byte[] buffer, int offset = 0) {
			var empty = new byte[] { 0, 0, 0, 0, 0, 0, 0, 0 };
			var des = new DESCrypto(empty, empty);
			return des.Decrypt(offset == 0 ? buffer : buffer.Sub(offset));
		}

		protected override void HandleSessionResponse(Packet packet) =>
			Send(AppPacket.Create(LoginOp.SessionReady, new SessionReady()));

		protected override void HandleAppPacket(AppPacket packet) {
			switch((LoginOp) packet.Opcode) {
				case LoginOp.ChatMessage:
					Send(AppPacket.Create(LoginOp.Login, new Login(), CryptoBlob));
					break;
				case LoginOp.LoginAccepted:
					if(packet.Data.Length < 90)
						LoginSuccess?.Invoke(this, false);
					else {
						var dec = Decrypt(packet.Data, 10);
						var rep = new LoginReply(dec);
						AccountID = rep.AcctID;
						SessionKey = rep.Key;
						LoginSuccess?.Invoke(this, true);
					}

					break;
				case LoginOp.ServerListResponse:
					var header = packet.Get<ServerListHeader>();
					ServerList?.Invoke(this, header.Servers);
					break;
				case LoginOp.PlayEverquestResponse:
					var resp = packet.Get<PlayResponse>();

					if(!resp.Allowed)
						CurPlay = null;

					PlaySuccess?.Invoke(this, CurPlay);
					break;
				default:
					WriteLine($"Unhandled packet in LoginStream: {(LoginOp) packet.Opcode} (0x{packet.Opcode:X04})");
					Hexdump(packet.Data);
					break;
			}
		}

		public void RequestServerList() => 
			Send(AppPacket.Create(LoginOp.ServerListRequest, new byte[] { 0, 0, 0, 0 }));

		public void Play(ServerListElement server) {
			CurPlay = server;
			Send(AppPacket.Create(LoginOp.PlayEverquestRequest, new PlayRequest(5, server.RuntimeID)));
		}
	}
}