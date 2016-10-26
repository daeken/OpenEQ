using System;
using System.Net;
using System.Net.Sockets;
using System.Threading;
using static System.Console;

namespace OpenEQ.Network {
    public class AsyncUDPConnection : IDisposable {
        public event EventHandler<byte[]> Receive;
        UdpClient client;
        Thread receiverThread;
        public AsyncUDPConnection(string host, int port) {
            client = new UdpClient(host, port);
            receiverThread = new Thread(Receiver);
            receiverThread.Start();
        }

        void Receiver() {
            try {
                IPEndPoint ep = new IPEndPoint(IPAddress.Any, 0);
                while(true) {
                    var packet = client.Receive(ref ep);
                    Receive?.Invoke(this, packet);
                }
            } catch(ThreadAbortException) {
            }
        }

        public void Send(byte[] packet) {
            WriteLine("Sending packet");
            client.Send(packet, packet.Length);
        }

        public void Dispose() {
            receiverThread?.Abort();
            client?.Close();
        }
    }
}
