using System.Security.Cryptography;
using static System.Console;
using static System.Text.Encoding;
using static OpenEQ.Utility;
using System;
using System.IO;
using System.Reflection;

namespace OpenEQ.Network {
    public class LoginStream : EQStream {
        byte[] cryptoBlob;
        public LoginStream(string host, int port, string username, string password) : base(host, port) {
            cryptoBlob = Encrypt(username, password);
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
            WriteLine("Got session response in login.");
            Send(AppPacket.Create(LoginOp.SessionReady, new SessionReady()));
        }

        protected override void HandleAppPacket(AppPacket packet) {
            switch((LoginOp) packet.Opcode) {
                case LoginOp.ChatMessage:
                    WriteLine($"Got chat message");

                    Send(AppPacket.Create(LoginOp.Login, new Login(), cryptoBlob));
                    break;
                case LoginOp.LoginAccepted:
                    WriteLine("Got login accepted");

                    if(packet.Data.Length < 80)
                        WriteLine("Bad login");
                    else
                        WriteLine("Good login!");

                    Send(AppPacket.Create(LoginOp.ServerListRequest));
                    break;
                case LoginOp.ServerListResponse:
                    WriteLine("Got server list");
                    var header = packet.Get<ServerListHeader>();
                    var off = 5 * 4;
                    var data = packet.Data;
                    for(var i = 0; i < header.serverCount; ++i) {
                        var ent = new ServerListElement(data, ref off);
                        WriteLine(ent.longname);
                    }
                    break;
                default:
                    WriteLine($"Unhandled packet in LoginStream: {(LoginOp) packet.Opcode} (0x{packet.Opcode:X04})");
                    Hexdump(packet.Data);
                    break;
            }
        }
    }
}
