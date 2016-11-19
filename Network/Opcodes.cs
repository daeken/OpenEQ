using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenEQ.Network {
    public enum SessionOp : ushort {
        Request = 0x0001, 
        Response = 0x0002, 
        Disconnect = 0x0005, 
        Stats = 0x0007, 
        Ack = 0x0015, 
        OutOfOrder = 0x0011, 
        Single = 0x0009, 
        Fragment = 0x000d, 
        Combined = 0x0003
    }

    public enum LoginOp : ushort {
        SessionReady = 0x0001,
        Login = 0x0002,
        ServerListRequest = 0x0004,
        PlayEverquestRequest = 0x000d,
        PlayEverquestResponse = 0x0021,
        ChatMessage = 0x0016,
        LoginAccepted = 0x0017,
        ServerListResponse = 0x0018,
        Poll = 0x0029,
        EnterChat = 0x000f,
        PollResponse = 0x0011
    }

    public enum WorldOp : ushort {
        SendLoginInfo = 0x13da
    }
}
