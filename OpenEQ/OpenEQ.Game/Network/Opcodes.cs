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
        SendLoginInfo = 0x13da, 
        ApproveWorld = 0x86c7, 
        LogServer = 0x6f79, 
        SendCharInfo = 0x4200, 
        ExpansionInfo = 0x7e4d,
        GuildsList = 0x5b0b,  
        EnterWorld = 0x51b9, 
        PostEnterWorld = 0x5d32, 
        DeleteCharacter = 0x5ca5, 
        CharacterCreateRequest = 0x53a3, 
        CharacterCreate = 0x1b85, 
        RandomNameGenerator = 0x647a, 
        ApproveName = 0x4f1f, 
        MessageOfTheDay = 0x7629, 
        ZoneServerInfo = 0x1190, 
        WorldComplete = 0x441c, 
        SetChatServer = 0x7d90, 
        SetChatServer2 = 0x158f
    }
}
