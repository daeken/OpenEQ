//! Session and application opcodes for the RoF2 client.

/// Reliable-protocol opcodes. Sent big-endian, preceded by a zero byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum SessionOp {
    Request = 0x0001,
    Response = 0x0002,
    Combined = 0x0003,
    Disconnect = 0x0005,
    KeepAlive = 0x0006,
    StatRequest = 0x0007,
    StatResponse = 0x0008,
    /// A single reliable application packet.
    Single = 0x0009,
    Fragment = 0x000D,
    OutOfOrder = 0x0011,
    Ack = 0x0015,
    OutOfSession = 0x001D,
}

impl SessionOp {
    pub fn from_u16(value: u16) -> Option<Self> {
        Some(match value {
            0x0001 => SessionOp::Request,
            0x0002 => SessionOp::Response,
            0x0003 => SessionOp::Combined,
            0x0005 => SessionOp::Disconnect,
            0x0006 => SessionOp::KeepAlive,
            0x0007 => SessionOp::StatRequest,
            0x0008 => SessionOp::StatResponse,
            0x0009 => SessionOp::Single,
            0x000D => SessionOp::Fragment,
            0x0011 => SessionOp::OutOfOrder,
            0x0015 => SessionOp::Ack,
            0x001D => SessionOp::OutOfSession,
            _ => return None,
        })
    }
}

/// Login-server opcodes. One set serves SoD through RoF2 clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum LoginOp {
    SessionReady = 0x0001,
    Login = 0x0002,
    ServerListRequest = 0x0004,
    PlayEverquestRequest = 0x000d,
    PlayEverquestResponse = 0x0022,
    ChatMessage = 0x0017,
    LoginAccepted = 0x0018,
    ServerListResponse = 0x0019,
    Poll = 0x0029,
    LoginExpansionPacketData = 0x0031,
    EnterChat = 0x000f,
    PollResponse = 0x0011,
}

/// World-server opcodes (character select).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum WorldOp {
    SendLoginInfo = 0x7a09,
    ApproveWorld = 0x7499,
    LogServer = 0x7ceb,
    SendCharInfo = 0x00d2,
    ExpansionInfo = 0x590d,
    GuildsList = 0x507a,
    EnterWorld = 0x578f,
    PostEnterWorld = 0x6259,
    SendMaxCharacters = 0x5475,
    CharacterCreate = 0x6bbf,
    DeleteCharacter = 0x1808,
    ApproveName = 0x56a2,
    MessageOfTheDay = 0x0c22,
    SetChatServer = 0x1bc5,
    SetChatServer2 = 0x7eec,
    ZoneServerInfo = 0x4c44,
    WorldComplete = 0x4493,
    ZoneUnavailable = 0x4cb4,
    WorldClientReady = 0x23c1,
}

/// Zone-server opcodes. Only those needed to enter the world are listed so
/// far; the rest arrive as the zone layer grows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ZoneOp {
    ZoneEntry = 0x5089,
    PlayerProfile = 0x6506,
    ReqNewZone = 0x7887,
    NewZone = 0x1795,
    ZoneSpawns = 0x5237,
    TimeOfDay = 0x5070,
    Weather = 0x661e,
    ReqClientSpawn = 0x35fa,
    SendExpZonein = 0x5f8e,
    ClientReady = 0x345d,
    SpawnDoor = 0x7291,
    SendZonepoints = 0x69a4,
    ClientUpdate = 0x7dfc,
    HpUpdate = 0x2828,
    SpawnAppearance = 0x0971,
    NewSpawn = 0x6097,
    DeleteSpawn = 0x7280,
    Animation = 0x7177,
    Death = 0x6517,
    ZoneChange = 0x2d18,
    Logout = 0x4ac6,
    CharInventory = 0x5ca6,
}

/// Human-readable name for a world opcode, for logging.
pub fn world_op_name(opcode: u16) -> &'static str {
    match opcode {
        0x7a09 => "OP_SendLoginInfo",
        0x7499 => "OP_ApproveWorld",
        0x7ceb => "OP_LogServer",
        0x00d2 => "OP_SendCharInfo",
        0x590d => "OP_ExpansionInfo",
        0x507a => "OP_GuildsList",
        0x578f => "OP_EnterWorld",
        0x6259 => "OP_PostEnterWorld",
        0x5475 => "OP_SendMaxCharacters",
        0x6bbf => "OP_CharacterCreate",
        0x1808 => "OP_DeleteCharacter",
        0x56a2 => "OP_ApproveName",
        0x0c22 => "OP_MOTD",
        0x1bc5 => "OP_SetChatServer",
        0x7eec => "OP_SetChatServer2",
        0x4c44 => "OP_ZoneServerInfo",
        0x4493 => "OP_WorldComplete",
        0x4cb4 => "OP_ZoneUnavail",
        0x23c1 => "OP_WorldClientReady",
        _ => "OP_Unknown",
    }
}
