using System.IO;
using System.Runtime.InteropServices;

namespace OpenEQ.Network {
    struct CharacterSelectEntry {
        public int Level, HairStyle, HairColor, Beard, Face, PrimaryID, SecondaryID, Deity, Zone, Instance, Race, Class, EyeColor1, EyeColor2, BeardColor, DrakkinHeritage, DrakkinTattoo, DrakkinDetails;
        public bool Gender; // false/true == male/female
        public string Name;

        /*public CharacterSelectEntry(byte[] data, int offset) {
            using(var ms = new MemoryStream(data, offset, data.Length - offset)) {
                using(var br = new BinaryReader(ms)) {
                    Level = br.ReadByte();
                    HairStyle = br.ReadByte();
                }
            }
        }*/
    }
}
