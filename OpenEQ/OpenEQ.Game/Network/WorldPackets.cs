using System;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;

namespace OpenEQ.Network {
    struct CharacterSelectEntry {
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
                }
                offset += (int) ms.Position;
            }
        }
    }

    struct CharacterSelect {
        public CharacterSelectEntry[] Characters;

        public CharacterSelect(byte[] data) {
            var (charcount, totalchars) = (data.U32(0), data.U32(4));
            var off = 8;
            Characters = new CharacterSelectEntry[charcount];
            for(var i = 0; i < charcount; ++i) {
                Characters[i] = new CharacterSelectEntry(data, ref off);
            }
        }
    }
}
