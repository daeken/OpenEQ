using System;
using System.Runtime.InteropServices;

namespace OpenEQ.Network {
    [StructLayout(LayoutKind.Sequential, Pack = 0)]
    public struct SessionReady {
        uint unk1;
        uint unk2;
        uint unk3;
    }
}
