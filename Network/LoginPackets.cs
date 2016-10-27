using System;
using System.Runtime.InteropServices;

namespace OpenEQ.Network {
    public enum ServerStatus {
        Up = 0, 
        Down = 1, 
        Locked = 2
    }

    [StructLayout(LayoutKind.Sequential, Pack = 0)]
    public struct SessionReady {
        uint unk1, unk2, unk3;
    }

    [StructLayout(LayoutKind.Sequential, Pack = 0)]
    public struct Login {
        ushort unk1, unk2, unk3, unk4, unk5;
        // Crypto blob comes after this
    }

    [StructLayout(LayoutKind.Sequential, Pack = 0)]
    public struct ServerListHeader {
        uint unk1, unk2, unk3, unk4;
        public uint serverCount;
    }

    [StructLayout(LayoutKind.Sequential, Pack = 0)]
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
}
