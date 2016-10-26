using System;
using System.Runtime.InteropServices;

namespace OpenEQ.Network {
    [StructLayout(LayoutKind.Sequential, Pack = 0)]
    public struct SessionRequest {
        uint Unknown;
        public uint SessionID;
        uint MaxLength;

        public SessionRequest(uint sessID) {
            Unknown = 2;
            SessionID = sessID;
            MaxLength = 512;
        }
    }

    [Flags]
    public enum ValidationMode : byte {
        None = 0,
        Crc = 2
    };

    [Flags]
    public enum FilterMode : byte {
        None = 0,
        Compressed = 1,
        Encoded = 4
    };

    [StructLayout(LayoutKind.Sequential, Pack = 0)]
    public struct SessionResponse {
        public uint sessionId;
        public uint crcKey;
        public ValidationMode validationMode;
        public FilterMode filterMode;
        public byte unknownA;
        public uint maxLength;
    }
}