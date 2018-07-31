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
	public enum ServerStatus {
		Up = 0, 
		Locked = 4, 
		Down = 1
	}
	internal static class ServerStatus_Helper {
		internal static ServerStatus Unpack(this ServerStatus val, BinaryReader br) {
			switch(br.ReadUInt32()) {
				case 0: case 2:
					return ServerStatus.Up;
				case 4:
					return ServerStatus.Locked;
				default:
					return ServerStatus.Down;
			}
		}
	}

	public struct ServerListElement : IEQStruct {
		public string WorldIP;
		public uint ServerListID;
		public uint RuntimeID;
		public string Longname;
		public string Language;
		public string Region;
		public ServerStatus Status;
		public uint PlayersOnline;

		public ServerListElement(string WorldIP, uint ServerListID, uint RuntimeID, string Longname, string Language, string Region, ServerStatus Status, uint PlayersOnline) : this() {
			this.WorldIP = WorldIP;
			this.ServerListID = ServerListID;
			this.RuntimeID = RuntimeID;
			this.Longname = Longname;
			this.Language = Language;
			this.Region = Region;
			this.Status = Status;
			this.PlayersOnline = PlayersOnline;
		}

		public ServerListElement(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public ServerListElement(BinaryReader br) : this() {
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
			WorldIP = br.ReadString(-1);
			ServerListID = br.ReadUInt32();
			RuntimeID = br.ReadUInt32();
			Longname = br.ReadString(-1);
			Language = br.ReadString(-1);
			Region = br.ReadString(-1);
			Status = ((ServerStatus) 0).Unpack(br);
			PlayersOnline = br.ReadUInt32();
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
			bw.Write(WorldIP.ToBytes());
			bw.Write(ServerListID);
			bw.Write(RuntimeID);
			bw.Write(Longname.ToBytes());
			bw.Write(Language.ToBytes());
			bw.Write(Region.ToBytes());
			bw.Write((uint) Status);
			bw.Write(PlayersOnline);
		}

		public override string ToString() {
			var ret = "struct ServerListElement {\n";
			ret += "\tWorldIP = ";
			try {
				ret += $"{ Indentify(WorldIP) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tServerListID = ";
			try {
				ret += $"{ Indentify(ServerListID) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tRuntimeID = ";
			try {
				ret += $"{ Indentify(RuntimeID) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tLongname = ";
			try {
				ret += $"{ Indentify(Longname) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tLanguage = ";
			try {
				ret += $"{ Indentify(Language) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tRegion = ";
			try {
				ret += $"{ Indentify(Region) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tStatus = ";
			try {
				ret += $"{ Indentify(Status) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tPlayersOnline = ";
			try {
				ret += $"{ Indentify(PlayersOnline) }\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			return ret + "}";
		}
	}

	public struct LoginReply : IEQStruct {
		byte message;
		byte unk1;
		byte unk2;
		byte unk3;
		byte unk4;
		byte unk5;
		byte unk6;
		byte unk7;
		public uint AcctID;
		public string Key;
		public uint FailedAttempts;

		public LoginReply(uint AcctID, string Key, uint FailedAttempts) : this() {
			this.AcctID = AcctID;
			this.Key = Key;
			this.FailedAttempts = FailedAttempts;
		}

		public LoginReply(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public LoginReply(BinaryReader br) : this() {
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
			message = br.ReadByte();
			unk1 = br.ReadByte();
			unk2 = br.ReadByte();
			unk3 = br.ReadByte();
			unk4 = br.ReadByte();
			unk5 = br.ReadByte();
			unk6 = br.ReadByte();
			unk7 = br.ReadByte();
			AcctID = br.ReadUInt32();
			Key = br.ReadString(11);
			FailedAttempts = br.ReadUInt32();
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
			bw.Write(message);
			bw.Write(unk1);
			bw.Write(unk2);
			bw.Write(unk3);
			bw.Write(unk4);
			bw.Write(unk5);
			bw.Write(unk6);
			bw.Write(unk7);
			bw.Write(AcctID);
			bw.Write(Key.ToBytes(11));
			bw.Write(FailedAttempts);
		}

		public override string ToString() {
			var ret = "struct LoginReply {\n";
			ret += "\tAcctID = ";
			try {
				ret += $"{ Indentify(AcctID) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tKey = ";
			try {
				ret += $"{ Indentify(Key) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tFailedAttempts = ";
			try {
				ret += $"{ Indentify(FailedAttempts) }\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			return ret + "}";
		}
	}

	public struct SessionReady : IEQStruct {
		uint unk1;
		uint unk2;
		uint unk3;

		public SessionReady(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public SessionReady(BinaryReader br) : this() {
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
			unk1 = br.ReadUInt32();
			unk2 = br.ReadUInt32();
			unk3 = br.ReadUInt32();
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
			bw.Write(unk1);
			bw.Write(unk2);
			bw.Write(unk3);
		}

		public override string ToString() {
			var ret = "struct SessionReady {\n";
			return ret + "}";
		}
	}

	public struct ServerListHeader : IEQStruct {
		uint unk1;
		uint unk2;
		uint unk3;
		uint unk4;
		uint serverCount;
		public List<ServerListElement> Servers;

		public ServerListHeader(List<ServerListElement> Servers) : this() {
			this.Servers = Servers;
		}

		public ServerListHeader(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public ServerListHeader(BinaryReader br) : this() {
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
			unk1 = br.ReadUInt32();
			unk2 = br.ReadUInt32();
			unk3 = br.ReadUInt32();
			unk4 = br.ReadUInt32();
			serverCount = br.ReadUInt32();
			Servers = new List<ServerListElement>();
			for(var i = 0; i < serverCount; ++i) {
				Servers.Add(new ServerListElement(br));
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
			bw.Write(unk1);
			bw.Write(unk2);
			bw.Write(unk3);
			bw.Write(unk4);
			bw.Write(serverCount);
			for(var i = 0; i < serverCount; ++i) {
				Servers[i].Pack(bw);
			}
		}

		public override string ToString() {
			var ret = "struct ServerListHeader {\n";
			ret += "\tServers = ";
			try {
				ret += "{\n";
				for(int i = 0, e = Servers.Count; i < e; ++i)
					ret += $"\t\t{ Indentify(Servers[i], 2) }" + (i != e - 1 ? "," : "") + "\n";
				ret += "\t}\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			return ret + "}";
		}
	}

	public struct Login : IEQStruct {
		ushort unk1;
		ushort unk2;
		ushort unk3;
		ushort unk4;
		ushort unk5;

		public Login(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public Login(BinaryReader br) : this() {
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
			unk1 = br.ReadUInt16();
			unk2 = br.ReadUInt16();
			unk3 = br.ReadUInt16();
			unk4 = br.ReadUInt16();
			unk5 = br.ReadUInt16();
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
			bw.Write(unk1);
			bw.Write(unk2);
			bw.Write(unk3);
			bw.Write(unk4);
			bw.Write(unk5);
		}

		public override string ToString() {
			var ret = "struct Login {\n";
			return ret + "}";
		}
	}

	public struct PlayResponse : IEQStruct {
		public byte Sequence;
		uint unk1;
		uint unk2;
		byte unk3;
		public bool Allowed;
		public ushort Message;
		ushort unk4;
		byte unk5;
		public uint ServerRuntimeID;

		public PlayResponse(byte Sequence, bool Allowed, ushort Message, uint ServerRuntimeID) : this() {
			this.Sequence = Sequence;
			this.Allowed = Allowed;
			this.Message = Message;
			this.ServerRuntimeID = ServerRuntimeID;
		}

		public PlayResponse(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public PlayResponse(BinaryReader br) : this() {
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
			Sequence = br.ReadByte();
			unk1 = br.ReadUInt32();
			unk2 = br.ReadUInt32();
			unk3 = br.ReadByte();
			Allowed = br.ReadByte() != 0;
			Message = br.ReadUInt16();
			unk4 = br.ReadUInt16();
			unk5 = br.ReadByte();
			ServerRuntimeID = br.ReadUInt32();
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
			bw.Write(Sequence);
			bw.Write(unk1);
			bw.Write(unk2);
			bw.Write(unk3);
			bw.Write((byte) (Allowed ? 1 : 0));
			bw.Write(Message);
			bw.Write(unk4);
			bw.Write(unk5);
			bw.Write(ServerRuntimeID);
		}

		public override string ToString() {
			var ret = "struct PlayResponse {\n";
			ret += "\tSequence = ";
			try {
				ret += $"{ Indentify(Sequence) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tAllowed = ";
			try {
				ret += $"{ Indentify(Allowed) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tMessage = ";
			try {
				ret += $"{ Indentify(Message) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tServerRuntimeID = ";
			try {
				ret += $"{ Indentify(ServerRuntimeID) }\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			return ret + "}";
		}
	}

	public struct PlayRequest : IEQStruct {
		public ushort Sequence;
		uint unk1;
		uint unk2;
		public uint ServerRuntimeID;

		public PlayRequest(ushort Sequence, uint ServerRuntimeID) : this() {
			this.Sequence = Sequence;
			this.ServerRuntimeID = ServerRuntimeID;
		}

		public PlayRequest(byte[] data, int offset = 0) : this() {
			Unpack(data, offset);
		}
		public PlayRequest(BinaryReader br) : this() {
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
			Sequence = br.ReadUInt16();
			unk1 = br.ReadUInt32();
			unk2 = br.ReadUInt32();
			ServerRuntimeID = br.ReadUInt32();
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
			bw.Write(Sequence);
			bw.Write(unk1);
			bw.Write(unk2);
			bw.Write(ServerRuntimeID);
		}

		public override string ToString() {
			var ret = "struct PlayRequest {\n";
			ret += "\tSequence = ";
			try {
				ret += $"{ Indentify(Sequence) },\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			ret += "\tServerRuntimeID = ";
			try {
				ret += $"{ Indentify(ServerRuntimeID) }\n";
			} catch(NullReferenceException) {
				ret += "!!NULL!!\n";
			}
			return ret + "}";
		}
	}
}
