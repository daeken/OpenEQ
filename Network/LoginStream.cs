using System.Security.Cryptography;
using static System.Console;
using static System.Text.Encoding;
using static OpenEQ.Utility;
using System;
using System.IO;
using System.Reflection;
using System.Collections.Generic;

namespace OpenEQ.Network {
    public class LoginStream : EQStream {
        public event EventHandler<bool> LoginSuccess;
        public event EventHandler<List<ServerListElement>> ServerList;

        byte[] cryptoBlob;
        public LoginStream(string host, int port) : base(host, port) {
        }

        public void Login(string username, string password) {
            cryptoBlob = Encrypt(username, password);
            Connect();
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
            using(var des = new DESCryptoServiceProvider()) {
                des.IV = new byte[] { 0, 0, 0, 0, 0, 0, 0, 0 };
                des.Mode = CipherMode.CBC;
                des.Padding = PaddingMode.Zeros;
                // Get around the restrictions on weak keys
                var meth = des.GetType().GetMethod("_NewEncryptor", BindingFlags.NonPublic | BindingFlags.Instance);
                var par = new object[] { new byte[] { 0, 0, 0, 0, 0, 0, 0, 0 }, des.Mode, des.IV, des.FeedbackSize, 0 };
                var crypt = meth.Invoke(des, par) as ICryptoTransform;
                using(var ms = new MemoryStream()) {
                    using(var cs = new CryptoStream(ms, crypt, CryptoStreamMode.Write)) {
                        cs.Write(buffer, 0, buffer.Length);
                    }
                    return ms.ToArray();
                }
            }
        }

        protected override void HandleSessionResponse(Packet packet) {
            Send(AppPacket.Create(LoginOp.SessionReady, new SessionReady()));
        }

        protected override void HandleAppPacket(AppPacket packet) {
            switch((LoginOp) packet.Opcode) {
                case LoginOp.ChatMessage:
                    Send(AppPacket.Create(LoginOp.Login, new Login(), cryptoBlob));
                    break;
                case LoginOp.LoginAccepted:
                    LoginSuccess?.Invoke(this, packet.Data.Length >= 80);
                    break;
                case LoginOp.ServerListResponse:
                    var header = packet.Get<ServerListHeader>();
                    var off = 5 * 4;
                    var data = packet.Data;
                    var serverList = new List<ServerListElement>();
                    for(var i = 0; i < header.serverCount; ++i)
                        serverList.Add(new ServerListElement(data, ref off));
                    ServerList?.Invoke(this, serverList);
                    break;
                default:
                    WriteLine($"Unhandled packet in LoginStream: {(LoginOp) packet.Opcode} (0x{packet.Opcode:X04})");
                    Hexdump(packet.Data);
                    break;
            }
        }

        public void RequestServerList() {
            Send(AppPacket.Create(LoginOp.ServerListRequest));
        }
    }
}
