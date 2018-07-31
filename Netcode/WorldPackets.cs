/*
*       o__ __o       o__ __o__/_   o          o    o__ __o__/_   o__ __o                o    ____o__ __o____   o__ __o__/_   o__ __o      
*      /v     v\     <|    v       <|\        <|>  <|    v       <|     v\              <|>    /   \   /   \   <|    v       <|     v\     
*     />       <\    < >           / \o      / \  < >           / \     <\             / \         \o/        < >           / \     <\    
*   o/                |            \o/ v\     \o/   |            \o/     o/           o/   \o        |          |            \o/       \o  
*  <|       _\__o__   o__/_         |   <\     |    o__/_         |__  _<|           <|__ __|>      < >         o__/_         |         |> 
*   \          |     |            / \    \o  / \   |             |       \          /       \       |          |            / \       //  
*     \         /    <o>           \o/     v\ \o/  <o>           <o>       \o      o/         \o     o         <o>           \o/      /    
*      o       o      |             |       <\ |    |             |         v\    /v           v\   <|          |             |      o     
*      <\__ __/>     / \  _\o__/_  / \        < \  / \  _\o__/_  / \         <\  />             <\  / \        / \  _\o__/_  / \  __/>     
*
* THIS FILE IS GENERATED BY structgen.py/structs.yml
* DO NOT EDIT
*
*/
using System;
using System.Collections.Generic;
using System.IO;
using static OpenEQ.Netcode.Utility;

namespace OpenEQ.Netcode {
	public struct EnterWorld : IEQStruct {
		public string Name;
		public bool Tutorial;
		public bool GoHome;

		public EnterWorld(string Name, bool Tutorial, bool GoHome) : this() {
			this.Name = Name;
			this.Tutorial = Tutorial;
			this.GoHome = GoHome;
		}

		public EnterWorld(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public EnterWorld(BinaryReader br) : this() {
			Unpack(br);
		}
		public void Unpack(byte[] data, int offset = 0) {
			using(var ms = new MemoryStream(data, offset, data.Length - offset)) {
				using(var br = new BinaryReader(ms)) {
					Unpack(br);
				}
			}
		}
		public void Unpack(BinaryReader br) {
			Name = br.ReadString(64);
			Tutorial = br.ReadUInt32() != 0;
			GoHome = br.ReadUInt32() != 0;
		}

		public byte[] Pack() {
			using(var ms = new MemoryStream()) {
				using(var bw = new BinaryWriter(ms)) {
					Pack(bw);
					return ms.ToArray();
				}
			}
		}
		public void Pack(BinaryWriter bw) {
			bw.Write(Name.ToBytes(64));
			bw.Write((uint) (Tutorial ? 1 : 0));
			bw.Write((uint) (GoHome ? 1 : 0));
		}

		public override string ToString() {
			var ret = "struct EnterWorld {\n";
			ret += "\tName = ";
			try {
				ret += $"{ Indentify(Name) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tTutorial = ";
			try {
				ret += $"{ Indentify(Tutorial) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tGoHome = ";
			try {
				ret += $"{ Indentify(GoHome) }\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			return ret + "}";
		}
	}

	public struct CharacterSelectEntry : IEQStruct {
		public byte Level;
		public byte HairStyle;
		public bool Gender;
		public string Name;
		public byte Beard;
		public byte HairColor;
		public byte Face;
		byte[] equipment;
		public uint PrimaryID;
		public uint SecondaryID;
		byte unknown15;
		public uint Deity;
		public ushort Zone;
		public ushort Instance;
		public bool GoHome;
		byte unknown19;
		public uint Race;
		public bool Tutorial;
		public byte Class;
		public byte EyeColor1;
		public byte BeardColor;
		public byte EyeColor2;
		public uint DrakkinHeritage;
		public uint DrakkinTattoo;
		public uint DrakkinDetails;
		byte unknown;

		public CharacterSelectEntry(byte Level, byte HairStyle, bool Gender, string Name, byte Beard, byte HairColor, byte Face, uint PrimaryID, uint SecondaryID, uint Deity, ushort Zone, ushort Instance, bool GoHome, uint Race, bool Tutorial, byte Class, byte EyeColor1, byte BeardColor, byte EyeColor2, uint DrakkinHeritage, uint DrakkinTattoo, uint DrakkinDetails) : this() {
			this.Level = Level;
			this.HairStyle = HairStyle;
			this.Gender = Gender;
			this.Name = Name;
			this.Beard = Beard;
			this.HairColor = HairColor;
			this.Face = Face;
			this.PrimaryID = PrimaryID;
			this.SecondaryID = SecondaryID;
			this.Deity = Deity;
			this.Zone = Zone;
			this.Instance = Instance;
			this.GoHome = GoHome;
			this.Race = Race;
			this.Tutorial = Tutorial;
			this.Class = Class;
			this.EyeColor1 = EyeColor1;
			this.BeardColor = BeardColor;
			this.EyeColor2 = EyeColor2;
			this.DrakkinHeritage = DrakkinHeritage;
			this.DrakkinTattoo = DrakkinTattoo;
			this.DrakkinDetails = DrakkinDetails;
		}

		public CharacterSelectEntry(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public CharacterSelectEntry(BinaryReader br) : this() {
			Unpack(br);
		}
		public void Unpack(byte[] data, int offset = 0) {
			using(var ms = new MemoryStream(data, offset, data.Length - offset)) {
				using(var br = new BinaryReader(ms)) {
					Unpack(br);
				}
			}
		}
		public void Unpack(BinaryReader br) {
			Level = br.ReadByte();
			HairStyle = br.ReadByte();
			Gender = br.ReadByte() != 0;
			Name = br.ReadString(-1);
			Beard = br.ReadByte();
			HairColor = br.ReadByte();
			Face = br.ReadByte();
			equipment = new byte[9 * 4 * 4];
			for(var i = 0; i < 9 * 4 * 4; ++i) {
				equipment[i] = br.ReadByte();
			}
			PrimaryID = br.ReadUInt32();
			SecondaryID = br.ReadUInt32();
			unknown15 = br.ReadByte();
			Deity = br.ReadUInt32();
			Zone = br.ReadUInt16();
			Instance = br.ReadUInt16();
			GoHome = br.ReadByte() != 0;
			unknown19 = br.ReadByte();
			Race = br.ReadUInt32();
			Tutorial = br.ReadByte() != 0;
			Class = br.ReadByte();
			EyeColor1 = br.ReadByte();
			BeardColor = br.ReadByte();
			EyeColor2 = br.ReadByte();
			DrakkinHeritage = br.ReadUInt32();
			DrakkinTattoo = br.ReadUInt32();
			DrakkinDetails = br.ReadUInt32();
			unknown = br.ReadByte();
		}

		public byte[] Pack() {
			using(var ms = new MemoryStream()) {
				using(var bw = new BinaryWriter(ms)) {
					Pack(bw);
					return ms.ToArray();
				}
			}
		}
		public void Pack(BinaryWriter bw) {
			bw.Write(Level);
			bw.Write(HairStyle);
			bw.Write((byte) (Gender ? 1 : 0));
			bw.Write(Name.ToBytes());
			bw.Write(Beard);
			bw.Write(HairColor);
			bw.Write(Face);
			for(var i = 0; i < 9 * 4 * 4; ++i) {
				bw.Write(equipment[i]);
			}
			bw.Write(PrimaryID);
			bw.Write(SecondaryID);
			bw.Write(unknown15);
			bw.Write(Deity);
			bw.Write(Zone);
			bw.Write(Instance);
			bw.Write((byte) (GoHome ? 1 : 0));
			bw.Write(unknown19);
			bw.Write(Race);
			bw.Write((byte) (Tutorial ? 1 : 0));
			bw.Write(Class);
			bw.Write(EyeColor1);
			bw.Write(BeardColor);
			bw.Write(EyeColor2);
			bw.Write(DrakkinHeritage);
			bw.Write(DrakkinTattoo);
			bw.Write(DrakkinDetails);
			bw.Write(unknown);
		}

		public override string ToString() {
			var ret = "struct CharacterSelectEntry {\n";
			ret += "\tLevel = ";
			try {
				ret += $"{ Indentify(Level) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tHairStyle = ";
			try {
				ret += $"{ Indentify(HairStyle) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tGender = ";
			try {
				ret += $"{ Indentify(Gender) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tName = ";
			try {
				ret += $"{ Indentify(Name) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tBeard = ";
			try {
				ret += $"{ Indentify(Beard) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tHairColor = ";
			try {
				ret += $"{ Indentify(HairColor) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tFace = ";
			try {
				ret += $"{ Indentify(Face) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tPrimaryID = ";
			try {
				ret += $"{ Indentify(PrimaryID) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tSecondaryID = ";
			try {
				ret += $"{ Indentify(SecondaryID) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tDeity = ";
			try {
				ret += $"{ Indentify(Deity) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tZone = ";
			try {
				ret += $"{ Indentify(Zone) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tInstance = ";
			try {
				ret += $"{ Indentify(Instance) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tGoHome = ";
			try {
				ret += $"{ Indentify(GoHome) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tRace = ";
			try {
				ret += $"{ Indentify(Race) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tTutorial = ";
			try {
				ret += $"{ Indentify(Tutorial) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tClass = ";
			try {
				ret += $"{ Indentify(Class) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tEyeColor1 = ";
			try {
				ret += $"{ Indentify(EyeColor1) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tBeardColor = ";
			try {
				ret += $"{ Indentify(BeardColor) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tEyeColor2 = ";
			try {
				ret += $"{ Indentify(EyeColor2) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tDrakkinHeritage = ";
			try {
				ret += $"{ Indentify(DrakkinHeritage) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tDrakkinTattoo = ";
			try {
				ret += $"{ Indentify(DrakkinTattoo) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tDrakkinDetails = ";
			try {
				ret += $"{ Indentify(DrakkinDetails) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			return ret + "}";
		}
	}

	public struct CharCreate : IEQStruct {
		public uint Class_;
		public uint Haircolor;
		public uint BeardColor;
		public uint Beard;
		public uint Gender;
		public uint Race;
		public uint StartZone;
		public uint HairStyle;
		public uint Deity;
		public uint STR;
		public uint STA;
		public uint AGI;
		public uint DEX;
		public uint WIS;
		public uint INT;
		public uint CHA;
		public uint Face;
		public uint EyeColor1;
		public uint EyeColor2;
		public uint DrakkinHeritage;
		public uint DrakkinTattoo;
		public uint DrakkinDetails;
		public uint Tutorial;

		public CharCreate(uint Class_, uint Haircolor, uint BeardColor, uint Beard, uint Gender, uint Race, uint StartZone, uint HairStyle, uint Deity, uint STR, uint STA, uint AGI, uint DEX, uint WIS, uint INT, uint CHA, uint Face, uint EyeColor1, uint EyeColor2, uint DrakkinHeritage, uint DrakkinTattoo, uint DrakkinDetails, uint Tutorial) : this() {
			this.Class_ = Class_;
			this.Haircolor = Haircolor;
			this.BeardColor = BeardColor;
			this.Beard = Beard;
			this.Gender = Gender;
			this.Race = Race;
			this.StartZone = StartZone;
			this.HairStyle = HairStyle;
			this.Deity = Deity;
			this.STR = STR;
			this.STA = STA;
			this.AGI = AGI;
			this.DEX = DEX;
			this.WIS = WIS;
			this.INT = INT;
			this.CHA = CHA;
			this.Face = Face;
			this.EyeColor1 = EyeColor1;
			this.EyeColor2 = EyeColor2;
			this.DrakkinHeritage = DrakkinHeritage;
			this.DrakkinTattoo = DrakkinTattoo;
			this.DrakkinDetails = DrakkinDetails;
			this.Tutorial = Tutorial;
		}

		public CharCreate(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public CharCreate(BinaryReader br) : this() {
			Unpack(br);
		}
		public void Unpack(byte[] data, int offset = 0) {
			using(var ms = new MemoryStream(data, offset, data.Length - offset)) {
				using(var br = new BinaryReader(ms)) {
					Unpack(br);
				}
			}
		}
		public void Unpack(BinaryReader br) {
			Class_ = br.ReadUInt32();
			Haircolor = br.ReadUInt32();
			BeardColor = br.ReadUInt32();
			Beard = br.ReadUInt32();
			Gender = br.ReadUInt32();
			Race = br.ReadUInt32();
			StartZone = br.ReadUInt32();
			HairStyle = br.ReadUInt32();
			Deity = br.ReadUInt32();
			STR = br.ReadUInt32();
			STA = br.ReadUInt32();
			AGI = br.ReadUInt32();
			DEX = br.ReadUInt32();
			WIS = br.ReadUInt32();
			INT = br.ReadUInt32();
			CHA = br.ReadUInt32();
			Face = br.ReadUInt32();
			EyeColor1 = br.ReadUInt32();
			EyeColor2 = br.ReadUInt32();
			DrakkinHeritage = br.ReadUInt32();
			DrakkinTattoo = br.ReadUInt32();
			DrakkinDetails = br.ReadUInt32();
			Tutorial = br.ReadUInt32();
		}

		public byte[] Pack() {
			using(var ms = new MemoryStream()) {
				using(var bw = new BinaryWriter(ms)) {
					Pack(bw);
					return ms.ToArray();
				}
			}
		}
		public void Pack(BinaryWriter bw) {
			bw.Write(Class_);
			bw.Write(Haircolor);
			bw.Write(BeardColor);
			bw.Write(Beard);
			bw.Write(Gender);
			bw.Write(Race);
			bw.Write(StartZone);
			bw.Write(HairStyle);
			bw.Write(Deity);
			bw.Write(STR);
			bw.Write(STA);
			bw.Write(AGI);
			bw.Write(DEX);
			bw.Write(WIS);
			bw.Write(INT);
			bw.Write(CHA);
			bw.Write(Face);
			bw.Write(EyeColor1);
			bw.Write(EyeColor2);
			bw.Write(DrakkinHeritage);
			bw.Write(DrakkinTattoo);
			bw.Write(DrakkinDetails);
			bw.Write(Tutorial);
		}

		public override string ToString() {
			var ret = "struct CharCreate {\n";
			ret += "\tClass_ = ";
			try {
				ret += $"{ Indentify(Class_) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tHaircolor = ";
			try {
				ret += $"{ Indentify(Haircolor) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tBeardColor = ";
			try {
				ret += $"{ Indentify(BeardColor) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tBeard = ";
			try {
				ret += $"{ Indentify(Beard) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tGender = ";
			try {
				ret += $"{ Indentify(Gender) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tRace = ";
			try {
				ret += $"{ Indentify(Race) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tStartZone = ";
			try {
				ret += $"{ Indentify(StartZone) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tHairStyle = ";
			try {
				ret += $"{ Indentify(HairStyle) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tDeity = ";
			try {
				ret += $"{ Indentify(Deity) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tSTR = ";
			try {
				ret += $"{ Indentify(STR) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tSTA = ";
			try {
				ret += $"{ Indentify(STA) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tAGI = ";
			try {
				ret += $"{ Indentify(AGI) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tDEX = ";
			try {
				ret += $"{ Indentify(DEX) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tWIS = ";
			try {
				ret += $"{ Indentify(WIS) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tINT = ";
			try {
				ret += $"{ Indentify(INT) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tCHA = ";
			try {
				ret += $"{ Indentify(CHA) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tFace = ";
			try {
				ret += $"{ Indentify(Face) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tEyeColor1 = ";
			try {
				ret += $"{ Indentify(EyeColor1) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tEyeColor2 = ";
			try {
				ret += $"{ Indentify(EyeColor2) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tDrakkinHeritage = ";
			try {
				ret += $"{ Indentify(DrakkinHeritage) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tDrakkinTattoo = ";
			try {
				ret += $"{ Indentify(DrakkinTattoo) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tDrakkinDetails = ";
			try {
				ret += $"{ Indentify(DrakkinDetails) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tTutorial = ";
			try {
				ret += $"{ Indentify(Tutorial) }\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			return ret + "}";
		}
	}

	public struct NameApproval : IEQStruct {
		public string Name;
		public uint Race;
		public uint Class;
		public uint Unknown;

		public NameApproval(string Name, uint Race, uint Class, uint Unknown) : this() {
			this.Name = Name;
			this.Race = Race;
			this.Class = Class;
			this.Unknown = Unknown;
		}

		public NameApproval(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public NameApproval(BinaryReader br) : this() {
			Unpack(br);
		}
		public void Unpack(byte[] data, int offset = 0) {
			using(var ms = new MemoryStream(data, offset, data.Length - offset)) {
				using(var br = new BinaryReader(ms)) {
					Unpack(br);
				}
			}
		}
		public void Unpack(BinaryReader br) {
			Name = br.ReadString(64);
			Race = br.ReadUInt32();
			Class = br.ReadUInt32();
			Unknown = br.ReadUInt32();
		}

		public byte[] Pack() {
			using(var ms = new MemoryStream()) {
				using(var bw = new BinaryWriter(ms)) {
					Pack(bw);
					return ms.ToArray();
				}
			}
		}
		public void Pack(BinaryWriter bw) {
			bw.Write(Name.ToBytes(64));
			bw.Write(Race);
			bw.Write(Class);
			bw.Write(Unknown);
		}

		public override string ToString() {
			var ret = "struct NameApproval {\n";
			ret += "\tName = ";
			try {
				ret += $"{ Indentify(Name) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tRace = ";
			try {
				ret += $"{ Indentify(Race) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tClass = ";
			try {
				ret += $"{ Indentify(Class) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tUnknown = ";
			try {
				ret += $"{ Indentify(Unknown) }\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			return ret + "}";
		}
	}

	public struct CharacterSelect : IEQStruct {
		uint charCount;
		uint totalChars;
		public List<CharacterSelectEntry> Characters;

		public CharacterSelect(List<CharacterSelectEntry> Characters) : this() {
			this.Characters = Characters;
		}

		public CharacterSelect(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public CharacterSelect(BinaryReader br) : this() {
			Unpack(br);
		}
		public void Unpack(byte[] data, int offset = 0) {
			using(var ms = new MemoryStream(data, offset, data.Length - offset)) {
				using(var br = new BinaryReader(ms)) {
					Unpack(br);
				}
			}
		}
		public void Unpack(BinaryReader br) {
			charCount = br.ReadUInt32();
			totalChars = br.ReadUInt32();
			Characters = new List<CharacterSelectEntry>();
			for(var i = 0; i < charCount; ++i) {
				Characters.Add(new CharacterSelectEntry(br));
			}
		}

		public byte[] Pack() {
			using(var ms = new MemoryStream()) {
				using(var bw = new BinaryWriter(ms)) {
					Pack(bw);
					return ms.ToArray();
				}
			}
		}
		public void Pack(BinaryWriter bw) {
			bw.Write(charCount);
			bw.Write(totalChars);
			for(var i = 0; i < charCount; ++i) {
				Characters[i].Pack(bw);
			}
		}

		public override string ToString() {
			var ret = "struct CharacterSelect {\n";
			ret += "\tCharacters = ";
			try {
				ret += "{\n";
				for(int i = 0, e = Characters.Count; i < e; ++i)
					ret += $"\t\t{ Indentify(Characters[i], 2) }" + (i != e - 1 ? "," : "") + "\n";
				ret += "\t}\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			return ret + "}";
		}
	}

	public struct ZoneServerInfo : IEQStruct {
		public string Host;
		public ushort Port;

		public ZoneServerInfo(string Host, ushort Port) : this() {
			this.Host = Host;
			this.Port = Port;
		}

		public ZoneServerInfo(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public ZoneServerInfo(BinaryReader br) : this() {
			Unpack(br);
		}
		public void Unpack(byte[] data, int offset = 0) {
			using(var ms = new MemoryStream(data, offset, data.Length - offset)) {
				using(var br = new BinaryReader(ms)) {
					Unpack(br);
				}
			}
		}
		public void Unpack(BinaryReader br) {
			Host = br.ReadString(128);
			Port = br.ReadUInt16();
		}

		public byte[] Pack() {
			using(var ms = new MemoryStream()) {
				using(var bw = new BinaryWriter(ms)) {
					Pack(bw);
					return ms.ToArray();
				}
			}
		}
		public void Pack(BinaryWriter bw) {
			bw.Write(Host.ToBytes(128));
			bw.Write(Port);
		}

		public override string ToString() {
			var ret = "struct ZoneServerInfo {\n";
			ret += "\tHost = ";
			try {
				ret += $"{ Indentify(Host) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tPort = ";
			try {
				ret += $"{ Indentify(Port) }\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			return ret + "}";
		}
	}
}
