using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;

namespace OpenEQ.Network {
    public struct CharacterSelectEntry {
        public int Level, HairStyle, HairColor, Beard, Face, PrimaryID, SecondaryID, Deity, Zone, Instance, Race, Class, EyeColor1, EyeColor2, BeardColor, DrakkinHeritage, DrakkinTattoo, DrakkinDetails;
        public bool Gender; // false/true == male/female
        public bool GoHome, Tutorial;
        public string Name;

        public CharacterSelectEntry(byte[] data, ref int offset) {
            using(var ms = new MemoryStream(data, offset, data.Length - offset)) {
                using(var br = new BinaryReader(ms)) {
                    (Level, HairStyle) = br.ReadByte(2);
                    Gender = br.ReadByte() == 1;
                    Name = br.ReadNullTermString();
                    (Beard, HairColor, Face) = br.ReadByte(3);
                    br.ReadBytes(9 * 4 * 4); // Skip equip
                    (PrimaryID, SecondaryID) = br.ReadInt32(2);
                    br.ReadByte(); // Unknown15
                    Deity = br.ReadInt32();
                    (Zone, Instance) = br.ReadInt16(2);
                    GoHome = br.ReadByte() == 1;
                    br.ReadByte(); // Unknown19
                    Race = br.ReadInt32();
                    Tutorial = br.ReadByte() == 1;
                    (Class, EyeColor1, BeardColor, EyeColor2) = br.ReadByte(4);
                    (DrakkinHeritage, DrakkinTattoo, DrakkinDetails) = br.ReadInt32(3);
                    br.ReadByte(); // Unknown
                    offset += (int) ms.Position;
                }
            }
        }
    }

    struct CharacterSelect {
        public List<CharacterSelectEntry> Characters;

        public CharacterSelect(byte[] data) {
            var (charcount, totalchars) = (data.U32(0), data.U32(4));
            var off = 8;
            Characters = new List<CharacterSelectEntry>();
            for(var i = 0; i < charcount; ++i) {
                Characters.Add(new CharacterSelectEntry(data, ref off));
            }
        }
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct EnterWorld {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 64)]
        public string Name;
        byte tutorial, goHome;

        public EnterWorld(string name, bool tutorial, bool goHome) {
            Name = name;
            this.tutorial = (byte) (tutorial ? 1 : 0);
            this.goHome = (byte) (goHome ? 1 : 0);
        }
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct ZoneServerInfo {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
        public string IP;
        public ushort Port;
    }
}
