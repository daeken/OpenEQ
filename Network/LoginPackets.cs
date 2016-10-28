using System;
using System.Runtime.InteropServices;

namespace OpenEQ.Network {
    public enum ServerStatus {
        Up = 0, 
        Down = 1, 
        Locked = 2
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct SessionReady {
        uint unk1, unk2, unk3;
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct Login {
        ushort unk1, unk2, unk3, unk4, unk5;
        // Crypto blob comes after this
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct ServerListHeader {
        uint unk1, unk2, unk3, unk4;
        public uint serverCount;
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct ServerListElement {
        public string worldIP;
        public uint serverListID;
        public uint runtimeID;
        public string longname;
        public string language; // "EN"
        public string region; // "US"
        public ServerStatus status;
        public uint playersOnline;

        public ServerListElement(byte[] data, ref int off) {
            worldIP = data.Str(ref off);
            serverListID = data.U32(ref off);
            runtimeID = data.U32(ref off);
            longname = data.Str(ref off);
            language = data.Str(ref off);
            region = data.Str(ref off);
            switch(data.U32(ref off)) {
                case 0: case 2:
                    status = ServerStatus.Up;
                    break;
                case 4:
                    status = ServerStatus.Locked;
                    break;
                default:
                    status = ServerStatus.Down;
                    break;
            }
            playersOnline = data.U32(ref off);
        }
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct PlayRequest {
        ushort sequence;
        uint unknown1, unknown2;
        uint serverRuntimeID;
        
        public PlayRequest(uint id) {
            sequence = 5;
            unknown1 = unknown2 = 0;
            serverRuntimeID = id;
        }
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct PlayResponse {
        byte sequence;
        uint unknown1, unknown2;
        byte unknown3;
        byte allowedFlag;
        ushort message;
        ushort unknown4;
        byte unknown5;
        public uint serverRuntimeID;

        public bool allowed => allowedFlag == 1;
    }
}
