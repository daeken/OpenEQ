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
using System.Collections.Generic;
using System.IO;
using static OpenEQ.Network.Utility;

namespace OpenEQ.Network {
	public enum Gender {
		Male = 0, 
		Female = 1
	}
	internal static class Gender_Helper {
		internal static Gender Unpack(this Gender val, BinaryReader br) {
			switch(br.ReadUInt32()) {
				case 0:
					return Gender.Male;
				default:
					return Gender.Female;
			}
		}
	}

	public struct ClientZoneEntry : IEQStruct {
		uint unk;
		public string CharName;

		public ClientZoneEntry(string CharName) : this() {
			this.CharName = CharName;
		}

		public ClientZoneEntry(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public ClientZoneEntry(BinaryReader br) : this() {
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
			unk = br.ReadUInt32();
			CharName = br.ReadString(64);
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
			bw.Write(unk);
			bw.Write(CharName.ToBytes(64));
		}

		public override string ToString() {
			var ret = "struct ClientZoneEntry {\n";
			ret += $"\tunk = { Indentify(unk) },\n";
			ret += $"\tCharName = { Indentify(CharName) }\n";
			return ret + "}";
		}
	}

	public struct PlayerProfile : IEQStruct {
		uint checksum;
		public Gender Gender;
		public uint Race;
		public uint Class;
		public byte Level;
		byte unkLevel;

		public PlayerProfile(Gender Gender, uint Race, uint Class, byte Level) : this() {
			this.Gender = Gender;
			this.Race = Race;
			this.Class = Class;
			this.Level = Level;
		}

		public PlayerProfile(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public PlayerProfile(BinaryReader br) : this() {
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
			checksum = br.ReadUInt32();
			Gender = ((Gender) 0).Unpack(br);
			Race = br.ReadUInt32();
			Class = br.ReadUInt32();
			br.ReadBytes(40);
			Level = br.ReadByte();
			unkLevel = br.ReadByte();
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
			bw.Write(checksum);
			bw.Write((uint) Gender);
			bw.Write(Race);
			bw.Write(Class);
			bw.Write(new byte[40]);
			bw.Write(Level);
			bw.Write(unkLevel);
		}

		public override string ToString() {
			var ret = "struct PlayerProfile {\n";
			ret += $"\tchecksum = { Indentify(checksum) },\n";
			ret += $"\tGender = { Indentify(Gender) },\n";
			ret += $"\tRace = { Indentify(Race) },\n";
			ret += $"\tClass = { Indentify(Class) },\n";
			ret += $"\tLevel = { Indentify(Level) },\n";
			ret += $"\tunkLevel = { Indentify(unkLevel) }\n";
			return ret + "}";
		}
	}
}
