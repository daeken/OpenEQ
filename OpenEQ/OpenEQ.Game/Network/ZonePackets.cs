using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;

namespace OpenEQ.Network {
    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct ClientZoneEntry {
        uint unknown;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 64)]
        public string CharName;

        public ClientZoneEntry(string name) {
            unknown = 0;
            CharName = name;
        }
    }
}
